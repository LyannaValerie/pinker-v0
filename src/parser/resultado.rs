use super::*;

impl Parser {
    // @pinker-nav:start parser.resultado.tentar-propagar
    // @pinker-nav:domain resultado
    // @pinker-nav:layer parser
    // @pinker-nav:summary Desugaring de `tentar` e `propagar`/`propagar?` sobre leques de resultado: reconhece os braços `sucesso`/`falha` (ou a forma curta de propagação) e abaixa para a mesma representação de `encaixe`, produzindo `ast::Stmt` sem caminho especial de runtime.
    /// Desugaring de `tentar` (Fase 223): tratamento estruturado sobre um
    /// leque de resultado declarado pelo usuário.
    ///
    /// ```text
    /// tentar expr {
    ///     sucesso Resultado.Ok(valor) { ... }
    ///     falha Resultado.Erro(erro) { ... }
    /// }
    /// ```
    ///
    /// O construto exige exatamente um braço `sucesso` e um braço `falha`, ambos
    /// apontando para variantes do mesmo leque. A execução abaixa para a mesma
    /// representação de `encaixe`, logo funciona no interpretador e no backend
    /// nativo sem caminho especial interpreter-only.
    pub(super) fn parse_tentar_desugared(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        self.consume(TokenKind::KwTentar, "tentar")?;
        let start_span = self.previous().span;
        let scrutinee = self.parse_expr()?;
        self.consume(TokenKind::LBrace, "{")?;

        struct TentarArm {
            variant: String,
            bindings: Vec<String>,
            body: Block,
            span: Span,
        }

        let mut enum_name: Option<String> = None;
        let mut arms: Vec<TentarArm> = Vec::new();
        let mut saw_success = false;
        let mut saw_failure = false;

        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            let label = self
                .consume(TokenKind::Ident, "'sucesso' ou 'falha' dentro de 'tentar'")?
                .clone();
            let label_span = label.span;
            match label.lexeme.as_str() {
                "sucesso" => {
                    if saw_success {
                        return Err(PinkerError::Parse {
                            msg: "tentar aceita apenas um braço 'sucesso'".to_string(),
                            span: label_span,
                        });
                    }
                    saw_success = true;
                }
                "falha" => {
                    if saw_failure {
                        return Err(PinkerError::Parse {
                            msg: "tentar aceita apenas um braço 'falha'".to_string(),
                            span: label_span,
                        });
                    }
                    saw_failure = true;
                }
                other => {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "esperado 'sucesso' ou 'falha' dentro de 'tentar', encontrado '{}'",
                            other
                        ),
                        span: label_span,
                    });
                }
            }

            let base = self
                .consume(TokenKind::Ident, "nome do leque no braço de tentar")?
                .lexeme
                .clone();
            self.consume(TokenKind::Dot, ".")?;
            let variant = self
                .consume(TokenKind::Ident, "nome da variante no braço de tentar")?
                .lexeme
                .clone();
            self.consume(TokenKind::LParen, "(")?;
            let mut bindings = Vec::new();
            if !self.check(TokenKind::RParen) {
                loop {
                    bindings.push(
                        self.consume(TokenKind::Ident, "nome da variável ligada pelo braço")?
                            .lexeme
                            .clone(),
                    );
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, ")")?;

            match &enum_name {
                None => enum_name = Some(base),
                Some(existing) if *existing == base => {}
                Some(existing) => {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "tentar mistura leques diferentes: '{}' e '{}'",
                            existing, base
                        ),
                        span: label_span,
                    });
                }
            }

            // Parte C: a categoria de coleção da variável ligada precisa existir
            // **antes** de o corpo do braço ser lido. O desugaring abaixo também
            // registra, mas registrar só lá é tarde demais: uma chamada como
            // `lista_tamanho(nomes)` dentro do corpo já teria sido resolvida
            // como `lista<bombom>` por falta de categoria.
            //
            // Latente até aqui porque as cargas de sucesso da Parte B eram
            // `verso` e `bombom` — nenhuma coleção passava por este caminho.
            if let Some(enum_ref) = enum_name.as_ref() {
                if let Some(variants) = self.enum_decls.get(enum_ref).cloned() {
                    if let Some((_, payloads)) = variants.iter().find(|(nome, _)| *nome == variant)
                    {
                        for (bind_name, payload_ty) in bindings.iter().zip(payloads.iter()) {
                            let (_, binding_ty) = self.payload_binding(payload_ty, label_span);
                            self.register_collection_type(bind_name, &binding_ty);
                        }
                    }
                }
            }

            // Parte G: as ligações do braço valem só dentro do corpo dele — é
            // lá que o desugaring emite os `Stmt::Let`. Mesmo motivo pelo qual
            // a categoria de coleção acima também precisa existir antes.
            self.abrir_escopo_local();
            for bind_name in &bindings {
                self.registrar_ligacao_local(bind_name);
            }
            let body_result = self.parse_block();
            self.fechar_escopo_local();
            let body = body_result?;
            arms.push(TentarArm {
                variant,
                bindings,
                body,
                span: label_span,
            });
        }
        self.consume(TokenKind::RBrace, "}")?;
        let end_span = self.previous().span;
        let helper_span = merge_span(start_span, end_span);

        if !saw_success || !saw_failure {
            return Err(PinkerError::Parse {
                msg: "tentar exige exatamente um braço 'sucesso' e um braço 'falha'".to_string(),
                span: helper_span,
            });
        }
        let Some(enum_name) = enum_name else {
            return Err(PinkerError::Parse {
                msg: "tentar exige braços com padrão Leque.Variante(...)".to_string(),
                span: helper_span,
            });
        };
        let Some(declared_variants) = self.enum_decls.get(&enum_name).cloned() else {
            return Err(PinkerError::Parse {
                msg: format!(
                    "tentar usa leque '{}' não declarado antes deste ponto",
                    enum_name
                ),
                span: helper_span,
            });
        };
        if !declared_variants
            .iter()
            .any(|(_, payloads)| !payloads.is_empty())
        {
            return Err(PinkerError::Parse {
                msg: "tentar exige leque com variantes de carga para transportar sucesso/falha"
                    .to_string(),
                span: helper_span,
            });
        }

        let mut seen_variants: Vec<&str> = Vec::new();
        for arm in &arms {
            let Some((_, payloads)) = declared_variants
                .iter()
                .find(|(name, _)| *name == arm.variant)
            else {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "variante '{}' não existe no leque '{}'",
                        arm.variant, enum_name
                    ),
                    span: arm.span,
                });
            };
            if seen_variants.contains(&arm.variant.as_str()) {
                return Err(PinkerError::Parse {
                    msg: format!("variante '{}' repetida no tentar", arm.variant),
                    span: arm.span,
                });
            }
            seen_variants.push(arm.variant.as_str());
            if payloads.is_empty() {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "variante '{}' não carrega valor; tentar exige carga explícita",
                        arm.variant
                    ),
                    span: arm.span,
                });
            }
            if payloads.len() != arm.bindings.len() {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "variante '{}' carrega {} valor(es), mas o braço liga {} nome(s)",
                        arm.variant,
                        payloads.len(),
                        arm.bindings.len()
                    ),
                    span: arm.span,
                });
            }
        }

        self.synthetic_counter += 1;
        let target_name = format!("__tentar_alvo_{}", self.synthetic_counter);
        let target_stmt = Stmt::Let(LetStmt {
            name: target_name.clone(),
            is_mut: false,
            ty: Some(Type::Alias {
                name: enum_name.clone(),
                span: helper_span,
            }),
            init: scrutinee,
            span: helper_span,
        });
        let target_ident = |span: Span| Expr {
            kind: ExprKind::Ident(target_name.clone()),
            span,
        };

        let mut else_branch: Option<ElseBlock> = None;
        for arm in arms.into_iter().rev() {
            let tag = declared_variants
                .iter()
                .position(|(name, _)| *name == arm.variant)
                .expect("variante validada acima") as u64;
            let condition = Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Call(
                            Box::new(Expr {
                                kind: ExprKind::Ident("__pinker_internal_leque_tag".to_string()),
                                span: arm.span,
                            }),
                            vec![target_ident(arm.span)],
                        ),
                        span: arm.span,
                    }),
                    BinaryOp::Eq,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(tag),
                        span: arm.span,
                    }),
                ),
                span: arm.span,
            };

            let payload_types = declared_variants
                .iter()
                .find(|(name, _)| *name == arm.variant)
                .map(|(_, payloads)| payloads.clone())
                .expect("variante validada acima");
            let mut body_stmts = Vec::new();
            for (index, (bind_name, payload_ty)) in
                arm.bindings.into_iter().zip(payload_types).enumerate()
            {
                let (carga_fn, binding_ty) = self.payload_binding(&payload_ty, arm.span);
                self.register_collection_type(&bind_name, &binding_ty);
                body_stmts.push(Stmt::Let(LetStmt {
                    name: bind_name,
                    is_mut: false,
                    ty: Some(binding_ty),
                    init: Expr {
                        kind: ExprKind::Call(
                            Box::new(Expr {
                                kind: ExprKind::Ident(carga_fn.to_string()),
                                span: arm.span,
                            }),
                            vec![
                                target_ident(arm.span),
                                Expr {
                                    kind: ExprKind::IntLit(tag),
                                    span: arm.span,
                                },
                                Expr {
                                    kind: ExprKind::IntLit(index as u64),
                                    span: arm.span,
                                },
                            ],
                        ),
                        span: arm.span,
                    },
                    span: arm.span,
                }));
            }
            body_stmts.extend(arm.body.stmts);
            let then_branch = Block {
                stmts: body_stmts,
                span: arm.body.span,
            };
            let if_stmt = IfStmt {
                condition,
                then_branch,
                else_branch,
                span: helper_span,
            };
            else_branch = Some(ElseBlock::If(Box::new(if_stmt)));
        }

        let Some(ElseBlock::If(root_if)) = else_branch else {
            unreachable!("tentar tem dois braços validados acima");
        };
        Ok(vec![target_stmt, Stmt::If(*root_if)])
    }

    /// Desugaring de `propagar` (Fases 224 e 237): retorno antecipado explícito
    /// para resultados baseados em leques.
    ///
    /// ```text
    /// propagar expr como Resultado.Ok(valor) senao Resultado.Erro(erro);
    /// propagar? expr como Resultado.Ok(valor);
    /// ```
    ///
    /// A variante de sucesso é validada e ignorada; a variante de falha tem sua
    /// carga extraída e retornada como `Resultado.Erro(carga)`. A sintaxe mantém
    /// o leque e as variantes explícitos para evitar inferência global prematura.
    /// A forma curta com `?` infere a falha apenas quando há exatamente uma
    /// outra variante com uma carga no mesmo leque.
    pub(super) fn parse_propagar_desugared(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        self.consume(TokenKind::KwPropagar, "propagar")?;
        let start_span = self.previous().span;
        let short_form = self.match_token(TokenKind::Question);
        let scrutinee = self.parse_expr()?;
        let como = self.consume(TokenKind::Ident, "'como' após expressão de propagar")?;
        if como.lexeme != "como" {
            return Err(PinkerError::Parse {
                msg: format!(
                    "esperado 'como' após expressão de propagar, encontrado '{}'",
                    como.lexeme
                ),
                span: como.span,
            });
        }
        let success_base = self
            .consume(TokenKind::Ident, "nome do leque no sucesso de propagar")?
            .lexeme
            .clone();
        self.consume(TokenKind::Dot, ".")?;
        let success_variant = self
            .consume(TokenKind::Ident, "nome da variante de sucesso em propagar")?
            .lexeme
            .clone();
        self.consume(TokenKind::LParen, "(")?;
        let success_binding = self
            .consume(
                TokenKind::Ident,
                "nome simbólico da carga de sucesso em propagar",
            )?
            .lexeme
            .clone();
        self.consume(TokenKind::RParen, ")")?;

        let (failure_base, failure_variant, failure_binding) = if short_form {
            self.consume(TokenKind::Semi, ";")?;
            self.synthetic_counter += 1;
            (
                success_base.clone(),
                String::new(),
                format!("__propagar_falha_{}", self.synthetic_counter),
            )
        } else {
            self.consume(TokenKind::KwSenao, "senao")?;
            let failure_base = self
                .consume(TokenKind::Ident, "nome do leque na falha de propagar")?
                .lexeme
                .clone();
            self.consume(TokenKind::Dot, ".")?;
            let failure_variant = self
                .consume(TokenKind::Ident, "nome da variante de falha em propagar")?
                .lexeme
                .clone();
            self.consume(TokenKind::LParen, "(")?;
            let failure_binding = self
                .consume(TokenKind::Ident, "nome da carga de falha em propagar")?
                .lexeme
                .clone();
            self.consume(TokenKind::RParen, ")")?;
            self.consume(TokenKind::Semi, ";")?;
            (failure_base, failure_variant, failure_binding)
        };
        let helper_span = merge_span(start_span, self.previous().span);

        if success_base != failure_base {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar mistura leques diferentes: '{}' e '{}'",
                    success_base, failure_base
                ),
                span: helper_span,
            });
        }
        let Some(declared_variants) = self.enum_decls.get(&success_base).cloned() else {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar usa leque '{}' não declarado antes deste ponto",
                    success_base
                ),
                span: helper_span,
            });
        };
        let failure_variant = if short_form {
            let candidates: Vec<String> = declared_variants
                .iter()
                .filter(|(name, payloads)| *name != success_variant && payloads.len() == 1)
                .map(|(name, _)| name.clone())
                .collect();
            match candidates.as_slice() {
                [only] => only.clone(),
                [] => {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "propagar? não encontrou variante de falha única com 1 carga no leque '{}'",
                            success_base
                        ),
                        span: helper_span,
                    });
                }
                many => {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "propagar? é ambíguo no leque '{}'; variantes candidatas: {}",
                            success_base,
                            many.join(", ")
                        ),
                        span: helper_span,
                    });
                }
            }
        } else {
            failure_variant
        };
        if success_variant == failure_variant {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar exige variantes distintas para sucesso e falha; '{}' foi repetida",
                    success_variant
                ),
                span: helper_span,
            });
        }
        let success_payloads = declared_variants
            .iter()
            .find(|(name, _)| *name == success_variant)
            .map(|(_, payloads)| payloads.clone())
            .ok_or_else(|| PinkerError::Parse {
                msg: format!(
                    "variante '{}' não existe no leque '{}'",
                    success_variant, success_base
                ),
                span: helper_span,
            })?;
        if success_payloads.len() != 1 {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar exige sucesso com exatamente 1 carga; variante '{}' tem {}",
                    success_variant,
                    success_payloads.len()
                ),
                span: helper_span,
            });
        }
        let failure_payloads = declared_variants
            .iter()
            .find(|(name, _)| *name == failure_variant)
            .map(|(_, payloads)| payloads.clone())
            .ok_or_else(|| PinkerError::Parse {
                msg: format!(
                    "variante '{}' não existe no leque '{}'",
                    failure_variant, success_base
                ),
                span: helper_span,
            })?;
        if failure_payloads.len() != 1 {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar exige falha com exatamente 1 carga; variante '{}' tem {}",
                    failure_variant,
                    failure_payloads.len()
                ),
                span: helper_span,
            });
        }

        self.synthetic_counter += 1;
        let target_name = format!("__propagar_alvo_{}", self.synthetic_counter);
        let target_stmt = Stmt::Let(LetStmt {
            name: target_name.clone(),
            is_mut: false,
            ty: Some(Type::Alias {
                name: success_base.clone(),
                span: helper_span,
            }),
            init: scrutinee,
            span: helper_span,
        });
        let failure_tag = declared_variants
            .iter()
            .position(|(name, _)| *name == failure_variant)
            .expect("variante validada acima") as u64;
        let success_tag = declared_variants
            .iter()
            .position(|(name, _)| *name == success_variant)
            .expect("variante validada acima") as u64;
        let target_ident = || Expr {
            kind: ExprKind::Ident(target_name.clone()),
            span: helper_span,
        };
        let success_declared_ty = success_payloads
            .into_iter()
            .next()
            .expect("validado exatamente uma carga");
        let failure_declared_ty = failure_payloads
            .into_iter()
            .next()
            .expect("validado exatamente uma carga");
        let (success_carga_fn, success_payload_ty) =
            self.payload_binding(&success_declared_ty, helper_span);
        let (failure_carga_fn, failure_payload_ty) =
            self.payload_binding(&failure_declared_ty, helper_span);
        self.register_collection_type(&success_binding, &success_payload_ty);
        self.register_collection_type(&failure_binding, &failure_payload_ty);
        let success_binding_stmt = Stmt::Let(LetStmt {
            name: success_binding,
            is_mut: false,
            ty: Some(success_payload_ty),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(success_carga_fn.to_string()),
                        span: helper_span,
                    }),
                    vec![
                        target_ident(),
                        Expr {
                            kind: ExprKind::IntLit(success_tag),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::IntLit(0),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let failure_binding_stmt = Stmt::Let(LetStmt {
            name: failure_binding.clone(),
            is_mut: false,
            ty: Some(failure_payload_ty),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(failure_carga_fn.to_string()),
                        span: helper_span,
                    }),
                    vec![
                        target_ident(),
                        Expr {
                            kind: ExprKind::IntLit(failure_tag),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::IntLit(0),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let return_stmt = Stmt::Return(ReturnStmt {
            expr: Some(Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::FieldAccess {
                            base: Box::new(Expr {
                                kind: ExprKind::Ident(success_base),
                                span: helper_span,
                            }),
                            field: failure_variant,
                        },
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(failure_binding),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            }),
            span: helper_span,
        });
        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: ExprKind::Ident("__pinker_internal_leque_tag".to_string()),
                            span: helper_span,
                        }),
                        vec![target_ident()],
                    ),
                    span: helper_span,
                }),
                BinaryOp::Eq,
                Box::new(Expr {
                    kind: ExprKind::IntLit(failure_tag),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };
        Ok(vec![
            target_stmt,
            Stmt::If(IfStmt {
                condition,
                then_branch: Block {
                    stmts: vec![failure_binding_stmt, return_stmt],
                    span: helper_span,
                },
                else_branch: None,
                span: helper_span,
            }),
            success_binding_stmt,
        ])
    }

    // @pinker-nav:end parser.resultado.tentar-propagar
}
