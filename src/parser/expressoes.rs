use super::*;

impl Parser {
    // @pinker-nav:start parser.expressoes.precedencia
    // @pinker-nav:domain expressoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Escada de precedência e operadores: `parse_expr`/`parse_expr_binary` com climbing por precedência e associatividade, e `parse_expr_unary`, produzindo `ast::Expr` com `BinaryOp`/`UnaryOp`.
    pub(super) fn parse_expr(&mut self) -> Result<Expr, PinkerError> {
        let expr = self.parse_expr_binary(0)?;
        if self.match_token(TokenKind::Question) {
            let then_expr = self.parse_expr()?;
            self.consume(TokenKind::Colon, ":")?;
            let else_expr = self.parse_expr()?;
            let span = merge_span(expr.span, else_expr.span);
            return Ok(Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("__ternario".to_string()),
                        span,
                    }),
                    vec![expr, then_expr, else_expr],
                ),
                span,
            });
        }
        Ok(expr)
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
            if token.kind == TokenKind::Minus
                || token.kind == TokenKind::Bang
                || token.kind == TokenKind::Star
                || token.kind == TokenKind::Amp
                || token.kind == TokenKind::Tilde
                || token.kind == TokenKind::KwNope
            {
                let op_span = token.span;
                let token_kind = token.kind;
                self.advance();
                let operand = if token_kind == TokenKind::Amp && self.check(TokenKind::Ident) {
                    let ident = self.advance().expect("identificador verificado").clone();
                    let mut name = ident.lexeme.clone();
                    let mut span = ident.span;
                    if self.match_token(TokenKind::Less) {
                        let mut type_args = Vec::new();
                        loop {
                            type_args.push(self.parse_type()?);
                            if !self.match_token(TokenKind::Comma) {
                                break;
                            }
                        }
                        self.consume(
                            TokenKind::Greater,
                            "> após argumentos de tipo no endereço de função",
                        )?;
                        let original_name = name.clone();
                        name = self.generic_function_name(&original_name, &type_args);
                        self.generic_instantiations.push(GenericInstantiation {
                            name: original_name,
                            type_args,
                            span: ident.span,
                        });
                        span = merge_span(span, self.previous().span);
                    }
                    Expr {
                        kind: ExprKind::Ident(name),
                        span,
                    }
                } else {
                    self.parse_expr_unary()?
                };
                if token_kind == TokenKind::Amp {
                    return Ok(Expr {
                        span: merge_span(op_span, operand.span),
                        kind: ExprKind::AddressOf(Box::new(operand)),
                    });
                }
                let unary_expr = Expr {
                    span: merge_span(op_span, operand.span),
                    kind: ExprKind::Unary(
                        if token_kind == TokenKind::Minus {
                            UnaryOp::Neg
                        } else if token_kind == TokenKind::Bang {
                            UnaryOp::Not
                        } else if token_kind == TokenKind::Tilde || token_kind == TokenKind::KwNope
                        {
                            UnaryOp::BitNot
                        } else {
                            UnaryOp::Deref
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

    // @pinker-nav:end parser.expressoes.precedencia

    // @pinker-nav:start parser.expressoes.primarias
    // @pinker-nav:domain expressoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Expressões primárias: literais, identificadores, agrupamento, listas/mapas e construção de struct/leque, produzindo o nó base `ast::Expr` antes da cadeia postfix.
    fn parse_expr_primary(&mut self) -> Result<Expr, PinkerError> {
        let eof_span = self.peek_span();
        let token = self
            .advance()
            .ok_or(PinkerError::Parse {
                msg: "fim inesperado da expressão".to_string(),
                span: eof_span,
            })?
            .clone();

        let base = match token.kind {
            TokenKind::IntLit => {
                let value = token
                    .lexeme
                    .parse::<u64>()
                    .map_err(|_| PinkerError::Parse {
                        msg: "literal inteiro fora da faixa de bombom/u64".to_string(),
                        span: token.span,
                    })?;
                Ok(Expr {
                    kind: ExprKind::IntLit(value),
                    span: token.span,
                })
            }
            TokenKind::KwVerdade => Ok(Expr {
                kind: ExprKind::BoolLit(true),
                span: token.span,
            }),
            TokenKind::KwFalso => Ok(Expr {
                kind: ExprKind::BoolLit(false),
                span: token.span,
            }),
            TokenKind::StringLit => Ok(Expr {
                kind: ExprKind::StringLit(token.lexeme.clone()),
                span: token.span,
            }),
            TokenKind::FStringLit => {
                let raw = token.lexeme.clone();
                let span = token.span;
                return self.desugar_fstring(&raw, span);
            }
            TokenKind::Ident => Ok(Expr {
                kind: ExprKind::Ident(token.lexeme.clone()),
                span: token.span,
            }),
            TokenKind::KwCarinho => self.parse_anonymous_function_expr(token.span),
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

    // @pinker-nav:end parser.expressoes.primarias

    // @pinker-nav:start parser.expressoes.postfix
    // @pinker-nav:domain expressoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Cadeia postfix de expressão: chamadas, acesso a campo, índice, chamada genérica explícita e sufixo de cast (`virar`), aplicados sobre a expressão base para produzir o `ast::Expr` final.
    fn parse_postfix_suffix(&mut self, mut expr: Expr) -> Result<Expr, PinkerError> {
        // #505: a grafia corrente veio da canonicalização de um membro de
        // módulo, e não do texto do usuário? É a única coisa que distingue
        // `arquivo.ler_bombom(...)` de alguém escrevendo `ler_arquivo(...)` a
        // seco.
        //
        // #532: esta distinção deixou de morrer aqui. Ela era local ao parser —
        // dentro deste laço as duas formas viravam o mesmo `Ident`, e a partir
        // daí toda camada tinha de readivinhar pelo texto. Agora ela é promovida
        // a `ExprKind::Intrinsic`, e é essa identidade que atravessa semantic,
        // IR, interpretador e backend nativo.
        let mut canonicalizado = false;
        loop {
            if let Some(generic_call) = self.try_parse_explicit_generic_call(&expr)? {
                expr = generic_call;
                continue;
            }
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
                // Parte G — CANONICALIZATION_BOUNDARY (forma seletiva).
                // Vem antes da monomorfização genérica e antes de
                // `falha_operacional::superficie`, de modo que a materialização
                // de `Resultado<T,E>` já enxergue a identidade canônica.
                if let ExprKind::Ident(name) = &expr.kind {
                    if let Some(canonica) = self.resolver_membro_seletivo(name) {
                        canonicalizado = true;
                        expr = Expr {
                            kind: ExprKind::Ident(canonica.to_string()),
                            span: expr.span,
                        };
                    }
                }
                // #505 — remoção da superfície global. Vem depois das duas
                // formas de import e antes de qualquer outra reescrita, de
                // modo que só chegue aqui grafia que o usuário escreveu sem
                // trazer nada.
                if !canonicalizado {
                    if let ExprKind::Ident(name) = &expr.kind {
                        self.recusar_intrinseca_sem_import(name, expr.span)?;
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    if let Some(template) = self.generic_templates.get(name).cloned() {
                        let type_args =
                            self.infer_generic_call_type_args(&template, &args, expr.span)?;
                        let mono_name = self.generic_function_name(name, &type_args);
                        self.generic_instantiations.push(GenericInstantiation {
                            name: name.clone(),
                            type_args,
                            span: expr.span,
                        });
                        expr = Expr {
                            kind: ExprKind::Ident(mono_name),
                            span: expr.span,
                        };
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    // Parte B: uma chamada a superfície falível materializa a
                    // especialização de `Resultado<T,E>` que ela devolve, pelo
                    // mesmo caminho de monomorfização que o usuário dispararia
                    // ao escrever o tipo. Materialização de tipo — não
                    // semântica de propagação, que continua cega à origem.
                    if let Some(superficie) = crate::falha_operacional::superficie(name) {
                        let span = expr.span;
                        self.registrar_resultado_falivel(superficie, span)?;
                    }
                    // Parte E1: `json_tipo` devolve o leque de classificação.
                    // Mesma regra das cargas da Parte C — o leque entra no
                    // programa quando a superfície que o produz é realmente
                    // chamada, e não porque existe um template predeclarado.
                    if name == crate::valor_json::intrinsecas::TIPO {
                        self.registrar_leque_predeclarado(crate::valor_json::LEQUE_TIPO_JSON);
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    let mut bindings = Vec::new();
                    let mut runtime_args = Vec::new();
                    for (index, arg) in args.into_iter().enumerate() {
                        let static_function = match &arg.kind {
                            ExprKind::Ident(arg_name) => self
                                .resolve_function_value_alias(arg_name)
                                .filter(|resolved| resolved != arg_name)
                                .or_else(|| {
                                    (arg_name.starts_with("__anon_carinho_")
                                        && !self.capturing_anon_functions.contains(arg_name))
                                    .then(|| arg_name.clone())
                                }),
                            _ => None,
                        };
                        if let Some(function_name) = static_function {
                            bindings.push(FunctionParamBinding {
                                index,
                                function_name,
                                span: arg.span,
                            });
                        } else {
                            runtime_args.push(arg);
                        }
                    }
                    if !bindings.is_empty() {
                        let mono_name = Self::function_param_specialization_name(name, &bindings);
                        self.function_param_instantiations
                            .push(FunctionParamInstantiation {
                                name: name.clone(),
                                bindings,
                                span: expr.span,
                            });
                        expr = Expr {
                            kind: ExprKind::Ident(mono_name),
                            span: expr.span,
                        };
                        args = runtime_args;
                    } else {
                        args = runtime_args;
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    if let Some(function_name) = self.resolve_function_value_alias(name) {
                        expr = Expr {
                            kind: ExprKind::Ident(function_name),
                            span: expr.span,
                        };
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    if let Some(first_arg) = args.first() {
                        if let ExprKind::Ident(map_name) = &first_arg.kind {
                            if let Some(kind) = self.collection_types.get(map_name.as_str()) {
                                if let Some(mono_name) = kind.generic_map_callee(name) {
                                    expr = Expr {
                                        kind: ExprKind::Ident(mono_name.to_string()),
                                        span: expr.span,
                                    };
                                }
                            }
                        }
                    }
                }
                expr = Self::promover_identidade_intrinseca(expr, canonicalizado);
                expr = Expr {
                    span: merge_span(expr.span, self.previous().span),
                    kind: ExprKind::Call(Box::new(expr), args),
                };
                continue;
            }
            if self.match_token(TokenKind::Dot) {
                let base_expr = expr;
                let field_token = self
                    .consume(TokenKind::Ident, "nome do campo após '.'")?
                    .clone();
                let field = field_token.lexeme.clone();
                // Parte G — CANONICALIZATION_BOUNDARY (forma qualificada).
                // `familia.membro` vira a identidade executiva ANTES de existir
                // como `FieldAccess`. Nada a jusante — nem o resto deste laço —
                // vê família ou membro.
                if let Some(canonica) = self.resolver_membro_de_familia(&base_expr, &field)? {
                    canonicalizado = true;
                    expr = Expr {
                        kind: ExprKind::Ident(canonica.to_string()),
                        span: merge_span(base_expr.span, field_token.span),
                    };
                    continue;
                }
                canonicalizado = false;
                expr = Expr {
                    span: merge_span(base_expr.span, field_token.span),
                    kind: ExprKind::FieldAccess {
                        base: Box::new(base_expr),
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
        Ok(Self::promover_identidade_intrinseca(expr, canonicalizado))
    }

    /// #532 — fim do CANONICALIZATION_BOUNDARY: a grafia vira identidade.
    ///
    /// Só chega aqui com `canonicalizado = true` a expressão cuja grafia foi
    /// produzida por um `trazer` resolvido — nunca a que o usuário escreveu. A
    /// promoção acontece no fim porque as reescritas intermediárias do postfix
    /// (monomorfização genérica de mapa/lista, materialização de `Resultado`,
    /// leque de `json_tipo`) ainda falam a linguagem da grafia canônica; o que
    /// muda é que a grafia final deixa de ser a autoridade e passa a viajar
    /// dentro da identidade.
    ///
    /// Uma grafia canonicalizada que a autoridade não reconheça como pública é
    /// forma interna do compilador e continua como `Ident`: `native_symbol` já
    /// impede que o usuário a declare, então ela não disputa nome com ninguém.
    /// #532 — callee intrínseco que o próprio compilador materializa.
    ///
    /// Açúcar de linguagem (`para cada`, interpolação `$"..."`) abaixa para
    /// chamadas de intrínsecas que o usuário não escreveu. Elas nascem já com
    /// identidade, e não com uma grafia que uma função homônima do usuário
    /// pudesse disputar.
    pub(super) fn callee_intrinseco(grafia: &str) -> ExprKind {
        ExprKind::Intrinsic(
            crate::intrinsics::identity::intrinsic_from_public_spelling(grafia)
                .expect("grafia intrínseca materializada pelo parser é pública"),
        )
    }

    fn promover_identidade_intrinseca(expr: Expr, canonicalizado: bool) -> Expr {
        if !canonicalizado {
            return expr;
        }
        let ExprKind::Ident(name) = &expr.kind else {
            return expr;
        };
        match crate::intrinsics::identity::callee_identity_da_grafia_canonica(name.as_str()) {
            crate::intrinsics::identity::CalleeIdentity::Intrinsic(identity) => Expr {
                kind: ExprKind::Intrinsic(identity),
                span: expr.span,
            },
            _ => expr,
        }
    }

    fn try_parse_explicit_generic_call(
        &mut self,
        expr: &Expr,
    ) -> Result<Option<Expr>, PinkerError> {
        let ExprKind::Ident(name) = &expr.kind else {
            return Ok(None);
        };
        if !self.check(TokenKind::Less) {
            return Ok(None);
        }
        let saved = self.current;
        self.advance();
        let mut type_args = Vec::new();
        loop {
            match self.parse_type() {
                Ok(ty) => type_args.push(ty),
                Err(_) => {
                    self.current = saved;
                    return Ok(None);
                }
            }
            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }
        if !self.match_token(TokenKind::Greater) || !self.match_token(TokenKind::LParen) {
            self.current = saved;
            return Ok(None);
        }

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
        let mono_name = self.generic_function_name(name, &type_args);
        self.generic_instantiations.push(GenericInstantiation {
            name: name.clone(),
            type_args,
            span: expr.span,
        });
        let callee = Expr {
            kind: ExprKind::Ident(mono_name),
            span: expr.span,
        };
        Ok(Some(Expr {
            span: merge_span(expr.span, self.previous().span),
            kind: ExprKind::Call(Box::new(callee), args),
        }))
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

    // @pinker-nav:end parser.expressoes.postfix

    // @pinker-nav:start parser.texto.interpolacao
    // @pinker-nav:domain texto
    // @pinker-nav:layer parser
    // @pinker-nav:summary Desugaring de strings interpoladas `$"..."`: reconhece os segmentos de texto e `{expr}`, parseia cada expressão embutida e produz uma chamada a `formatar_verso` — um `ast::Expr`.
    fn desugar_fstring(&mut self, raw: &str, span: Span) -> Result<Expr, PinkerError> {
        let mut template = String::new();
        let mut expr_sources: Vec<String> = Vec::new();
        let mut chars = raw.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' {
                let mut depth = 1u32;
                let mut expr_str = String::new();
                for inner in chars.by_ref() {
                    if inner == '{' {
                        depth += 1;
                    } else if inner == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    expr_str.push(inner);
                }
                if depth != 0 {
                    return Err(PinkerError::Parse {
                        msg: "'}' não encontrado em string interpolada".to_string(),
                        span,
                    });
                }
                template.push_str("{}");
                expr_sources.push(expr_str);
            } else {
                template.push(c);
            }
        }

        if expr_sources.is_empty() {
            return Ok(Expr {
                kind: ExprKind::StringLit(template),
                span,
            });
        }

        let mut call_args = vec![Expr {
            kind: ExprKind::StringLit(template),
            span,
        }];

        for src in &expr_sources {
            let mut lexer = Lexer::new(src);
            let tokens = lexer.tokenize().map_err(|e| PinkerError::Parse {
                msg: format!("erro ao lexar expressão em string interpolada: {}", e),
                span,
            })?;
            let mut sub_parser = Parser::new(tokens);
            let expr = sub_parser.parse_expr().map_err(|e| PinkerError::Parse {
                msg: format!("erro ao parsear expressão em string interpolada: {}", e),
                span,
            })?;
            call_args.push(expr);
        }

        Ok(Expr {
            kind: ExprKind::Call(
                Box::new(Expr {
                    kind: Self::callee_intrinseco("formatar_verso"),
                    span,
                }),
                call_args,
            ),
            span,
        })
    }
    // @pinker-nav:end parser.texto.interpolacao
}
