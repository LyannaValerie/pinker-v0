use super::*;

impl Parser {
    // @pinker-nav:start parser.lacos.for-each
    // @pinker-nav:domain lacos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Desugaring de `para cada X em COL { ... }`: reconhece a forma for-each e a reescreve em laço explícito com cursor/índice e chamadas de iteração conforme o tipo da coleção (listas e mapas, por chave/valor), produzindo `ast::Stmt`.
    pub(super) fn parse_for_stmt_desugared(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        let start_span = self.consume(TokenKind::KwPara, "para")?.span;
        if self.match_token(TokenKind::KwCada) {
            return self.parse_for_each_after_cada(start_span);
        }
        let var_name = self
            .consume(TokenKind::Ident, "variável do iterador em 'para'")?
            .lexeme
            .clone();
        // Parte G: a variável do laço precisa estar visível ANTES de o corpo
        // ser lido. O escopo é o bloco envolvente porque é exatamente lá que o
        // desugaring emite o `Stmt::Let` dela.
        self.registrar_ligacao_local(&var_name);
        self.consume(TokenKind::KwEm, "em")?;
        let start_expr = self.parse_expr()?;
        self.consume(TokenKind::DotDot, "..")?;
        let end_expr = self.parse_expr()?;
        let body = self.parse_block()?;
        let loop_span = merge_span(start_span, body.span);
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let limit_name = format!("__range_limite_{suffix}");

        let var_binding = Stmt::Let(LetStmt {
            name: var_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(loop_span)),
            init: start_expr,
            span: loop_span,
        });
        let limit_binding = Stmt::Let(LetStmt {
            name: limit_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(loop_span)),
            init: end_expr,
            span: loop_span,
        });
        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(var_name.clone()),
                    span: loop_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(limit_name),
                    span: loop_span,
                }),
            ),
            span: loop_span,
        };
        let increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(var_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(var_name),
                        span: loop_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: loop_span,
                    }),
                ),
                span: loop_span,
            },
            span: loop_span,
        });
        let mut while_body = body.stmts;
        while_body.push(increment);
        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body,
                span: loop_span,
            },
            span: loop_span,
        });
        Ok(vec![var_binding, limit_binding, while_stmt])
    }

    fn parse_for_each_after_cada(&mut self, start_span: Span) -> Result<Vec<Stmt>, PinkerError> {
        let item_name = self
            .consume(TokenKind::Ident, "variável do item em 'para cada'")?
            .lexeme
            .clone();
        self.consume(TokenKind::KwEm, "em")?;
        let collection_expr = self.parse_expr()?;
        // Parte G: o item do laço vale só dentro do corpo — é lá que o
        // desugaring emite o `Stmt::Let` dele. O escopo extra existe para que a
        // ligação não escape para o resto do bloco envolvente.
        self.abrir_escopo_local();
        self.registrar_ligacao_local(&item_name);
        let body_result = self.parse_block();
        self.fechar_escopo_local();
        let body = body_result?;
        let loop_span = merge_span(start_span, body.span);

        let collection_kind = match &collection_expr.kind {
            ExprKind::Ident(name) => self.collection_types.get(name.as_str()).cloned(),
            _ => None,
        };

        match collection_kind {
            Some(CollectionKind::MapVersoBombom) => {
                self.desugar_for_each_map(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::MapVersoVerso) => {
                self.desugar_for_each_map_verso_verso(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::MapBombomBombom) => {
                self.desugar_for_each_map_bombom_bombom(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::MapBombomVerso) => {
                self.desugar_for_each_map_bombom_verso(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::Map { key }) => {
                self.desugar_for_each_map_generic(item_name, key, collection_expr, body, loop_span)
            }
            Some(CollectionKind::ListVerso) => {
                self.desugar_for_each_list_verso(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::ListEnum(element)) => self.desugar_for_each_list_enum(
                item_name,
                element,
                collection_expr,
                body,
                loop_span,
            ),
            _ => self.desugar_for_each_list(item_name, collection_expr, body, loop_span),
        }
    }

    /// Desugaring de `para cada item em lista<Leque>` — usa as intrínsecas
    /// genéricas de lista (Fase 211) e liga o item com o tipo do leque.
    fn desugar_for_each_list_enum(
        &mut self,
        item_name: String,
        element: String,
        list_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let list_slot_name = format!("__iter_lista_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let list_binding_stmt = Stmt::Let(LetStmt {
            name: list_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: list_expr,
            span: helper_span,
        });
        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: Self::callee_intrinseco("lista_tamanho"),
                            span: helper_span,
                        }),
                        vec![Expr {
                            kind: ExprKind::Ident(list_slot_name.clone()),
                            span: helper_span,
                        }],
                    ),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let item_binding = Stmt::Let(LetStmt {
            name: item_name,
            is_mut: false,
            ty: Some(Type::Alias {
                name: element,
                span: helper_span,
            }),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: Self::callee_intrinseco("lista_obter"),
                        span: helper_span,
                    }),
                    vec![
                        Expr {
                            kind: ExprKind::Ident(list_slot_name),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::Ident(index_slot_name.clone()),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(item_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![list_binding_stmt, index_binding_stmt, while_stmt])
    }

    /// Desugaring de `para cada item em lista<bombom>` — reutilizado da Fase 153.
    fn desugar_for_each_list(
        &mut self,
        item_name: String,
        list_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let list_slot_name = format!("__iter_lista_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let list_binding_stmt = Stmt::Let(LetStmt {
            name: list_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: list_expr,
            span: helper_span,
        });
        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: Self::callee_intrinseco("lista_bombom_tamanho"),
                            span: helper_span,
                        }),
                        vec![Expr {
                            kind: ExprKind::Ident(list_slot_name.clone()),
                            span: helper_span,
                        }],
                    ),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let item_binding = Stmt::Let(LetStmt {
            name: item_name,
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: Self::callee_intrinseco("lista_bombom_obter"),
                        span: helper_span,
                    }),
                    vec![
                        Expr {
                            kind: ExprKind::Ident(list_slot_name),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::Ident(index_slot_name.clone()),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(item_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![list_binding_stmt, index_binding_stmt, while_stmt])
    }

    fn desugar_for_each_list_verso(
        &mut self,
        item_name: String,
        list_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let list_slot_name = format!("__iter_lista_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let list_binding_stmt = Stmt::Let(LetStmt {
            name: list_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: list_expr,
            span: helper_span,
        });
        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: Self::callee_intrinseco("lista_verso_tamanho"),
                            span: helper_span,
                        }),
                        vec![Expr {
                            kind: ExprKind::Ident(list_slot_name.clone()),
                            span: helper_span,
                        }],
                    ),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let item_binding = Stmt::Let(LetStmt {
            name: item_name,
            is_mut: false,
            ty: Some(Type::Verso(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: Self::callee_intrinseco("lista_verso_obter"),
                        span: helper_span,
                    }),
                    vec![
                        Expr {
                            kind: ExprKind::Ident(list_slot_name),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::Ident(index_slot_name.clone()),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(item_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![list_binding_stmt, index_binding_stmt, while_stmt])
    }

    /// Desugaring de `para cada chave em mapa<verso,bombom>` — Fase 155.
    ///
    /// Lowering auditável:
    /// ```text
    /// nova __iter_mapa_N    = mapa_expr;
    /// nova __iter_tamanho_N = mapa_verso_bombom_tamanho(__iter_mapa_N);
    /// nova __iter_cursor_N  = <cursor interno sobre snapshot de chaves>;
    /// nova muda __iter_indice_N: bombom = 0;
    /// enquanto __iter_indice_N < __iter_tamanho_N {
    ///     nova chave: verso = <próxima chave do cursor interno>;
    ///     __iter_indice_N = __iter_indice_N + 1;
    ///     <corpo>
    /// }
    /// ```
    fn desugar_for_each_map(
        &mut self,
        key_name: String,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        // nova __iter_mapa_N = map_expr;
        let map_binding_stmt = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });

        // nova __iter_tamanho_N: bombom = mapa_verso_bombom_tamanho(__iter_mapa_N);
        let size_binding_stmt = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: Self::callee_intrinseco("mapa_verso_bombom_tamanho"),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        // nova __iter_cursor_N: bombom = <cursor interno sobre snapshot de chaves>;
        let cursor_binding_stmt = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::InternalMapIterCreate(Box::new(Expr {
                    kind: ExprKind::Ident(map_slot_name.clone()),
                    span: helper_span,
                })),
                span: helper_span,
            },
            span: helper_span,
        });

        // nova muda __iter_indice_N: bombom = 0;
        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        // condição: __iter_indice_N < __iter_tamanho_N
        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        // nova key_name: verso = <próxima chave do cursor interno>;
        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(Type::Verso(helper_span)),
            init: Expr {
                kind: ExprKind::InternalMapIterNextKey(Box::new(Expr {
                    kind: ExprKind::Ident(cursor_slot_name),
                    span: helper_span,
                })),
                span: helper_span,
            },
            span: helper_span,
        });

        // __iter_indice_N = __iter_indice_N + 1;
        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(key_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![
            map_binding_stmt,
            size_binding_stmt,
            cursor_binding_stmt,
            index_binding_stmt,
            while_stmt,
        ])
    }

    /// Desugaring único para a autoridade adulta `mapa<K,V>`. O cursor e a
    /// operação de tamanho são independentes de V; apenas o tipo devolvido da
    /// chave acompanha K.
    fn desugar_for_each_map_generic(
        &mut self,
        key_name: String,
        key_ty: Type,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;
        let next_callee = if matches!(key_ty, Type::Verso(_)) {
            "__pinker_internal_mapa_iterador_proxima_chave_verso"
        } else {
            "__pinker_internal_mapa_iterador_proxima_chave_bombom"
        };

        let map_binding = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });
        let size_binding = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: Self::callee_intrinseco("mapa_tamanho"),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let cursor_binding = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("__pinker_internal_mapa_iterador_criar".to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let index_binding = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });
        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };
        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(key_ty),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(next_callee.to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(cursor_slot_name),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let mut while_body = Vec::with_capacity(2 + body.stmts.len());
        while_body.push(key_binding);
        while_body.push(increment);
        while_body.extend(body.stmts);
        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body,
                span: helper_span,
            },
            span: loop_span,
        });
        Ok(vec![
            map_binding,
            size_binding,
            cursor_binding,
            index_binding,
            while_stmt,
        ])
    }

    fn desugar_for_each_map_verso_verso(
        &mut self,
        key_name: String,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let map_binding_stmt = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });

        let size_binding_stmt = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: Self::callee_intrinseco("mapa_verso_verso_tamanho"),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let cursor_binding_stmt = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_verso_verso_iterador_criar".to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(Type::Verso(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_verso_verso_iterador_proxima_chave".to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(cursor_slot_name),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(key_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![
            map_binding_stmt,
            size_binding_stmt,
            cursor_binding_stmt,
            index_binding_stmt,
            while_stmt,
        ])
    }

    fn desugar_for_each_map_bombom_bombom(
        &mut self,
        key_name: String,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let map_binding_stmt = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });

        let size_binding_stmt = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: Self::callee_intrinseco("mapa_bombom_bombom_tamanho"),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let cursor_binding_stmt = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_bombom_bombom_iterador_criar".to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave"
                                .to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(cursor_slot_name),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(key_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![
            map_binding_stmt,
            size_binding_stmt,
            cursor_binding_stmt,
            index_binding_stmt,
            while_stmt,
        ])
    }

    fn desugar_for_each_map_bombom_verso(
        &mut self,
        key_name: String,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let map_binding_stmt = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });

        let size_binding_stmt = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: Self::callee_intrinseco("mapa_bombom_verso_tamanho"),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let cursor_binding_stmt = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_bombom_verso_iterador_criar".to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave"
                                .to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(cursor_slot_name),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(key_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![
            map_binding_stmt,
            size_binding_stmt,
            cursor_binding_stmt,
            index_binding_stmt,
            while_stmt,
        ])
    }

    // @pinker-nav:end parser.lacos.for-each
}
