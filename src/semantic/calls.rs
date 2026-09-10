//! Despacho de chamadas da checagem semântica, movido de `src/semantic.rs`
//! pela unidade SEM-1 do inventário da #601 (Task #619).
//!
//! Só o arquivo mudou: a região cartografada `semantic.chamadas.despacho`, as
//! seis funções que a compõem e a ordem em que decidem continuam exatamente
//! como estavam. `super` mudou de significado ao descer um nível, e o `use`
//! abaixo devolve ao irmão o vocabulário do pai — `SemanticChecker`, os tipos
//! da AST, os tipos de `method_dispatch` e os helpers privados — sem promover
//! nada: um filho enxerga os itens privados do pai por privacidade de módulo, e
//! este `use` é privado.
//!
//! A autoridade da seleção de método continua sendo `crate::method_dispatch`
//! (#590/#591, consolidação C2). Esta camada só constrói candidatos a partir de
//! `method_index` e traduz o veredito; nenhuma regra de precedência, desempate,
//! escolha de representante ou decisão por grafia nasce aqui.
//!
//! Nenhum item do corte era `pub` antes do move e nenhum é agora.
//! `check_call_expr` é o único símbolo que o pai chama e, por isso, o único que
//! passou de privado a `pub(super)`.

use super::*;

impl SemanticChecker {
    // @pinker-nav:start semantic.chamadas.despacho
    // @pinker-nav:domain chamadas
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Despacho de chamadas: resolução de método de impl (direta e qualificada por trato), restringida aos tratos que a unidade-fonte da chamada autorizou — uma chamada de método não nomeia o trato, então sem esse filtro um trato da raiz forneceria método default ao corpo de um módulo que nunca o importou. Quem alcança, quem precede e quem vence entre os candidatos não é decidido aqui: esta camada só constrói candidatos a partir de `method_index` e traduz o veredito de `method_dispatch`, a autoridade única que o lowering consulta com a mesma regra; a chamada qualificada nomeia o trato e continua sendo resolução de identidade, sem candidatos a comparar, e a correspondência exata dessa identidade é de `method_identity` desde a #647 — aqui sobram o adaptador que resolve o alvo e a mensagem, que é da fase; desde a #649 ela também pergunta o ALCANCE da relação resolvida à mesma autoridade de `module_resolve` que o despacho não qualificado consulta, porque poder nomear o trato não autoriza relação de unidade que este contexto nunca pediu. Também: seleção monomórfica das intrínsecas genéricas de mapa, checagem de chamada nomeada (aridade e tipos de argumento) e o despachante `check_call_expr` — construção de variante de leque, desugaring de `encaixe`, a checagem genérica das grafias históricas de contrato declarado, dirigida por `intrinsics::registry`, e os contratos próprios que sobram (aridade variável, formas genéricas de lista/mapa e restrições que não cabem em `(params, ret)`), caindo para a chamada de função declarada.
    fn check_trait_object_method_call(
        &mut self,
        expr_span: Span,
        callee_span: Span,
        trait_name: &str,
        method_name: &str,
        args: &[Expr],
    ) -> Result<Type, PinkerError> {
        let (method_params, method_ret_type) = {
            let trait_decl = self
                .traits
                .get(trait_name)
                .ok_or_else(|| PinkerError::Semantic {
                    msg: format!("trato '{}' não declarado", trait_name),
                    span: callee_span,
                })?;

            let method = trait_decl
                .methods
                .iter()
                .find(|method| method.name == method_name)
                .ok_or_else(|| PinkerError::Semantic {
                    msg: format!(
                        "método '{}' não existe no trato objetificável '{}'",
                        method_name, trait_name
                    ),
                    span: callee_span,
                })?;

            (
                method.params.iter().skip(1).cloned().collect::<Vec<_>>(),
                method.ret_type.clone(),
            )
        };

        if args.len() != method_params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "chamada dinâmica de '{}.{}' com aridade inválida: esperado {}, recebido {}",
                    trait_name,
                    method_name,
                    method_params.len(),
                    args.len()
                ),
                span: expr_span,
            });
        }

        for (index, (arg, expected)) in args.iter().zip(method_params.iter()).enumerate() {
            let arg_ty = self.check_value_expr(
                arg,
                "resultado de função sem retorno não pode ser usado como argumento de método dinâmico",
            )?;
            let expected_ty = self.resolve_type_or_error(&expected.ty)?;

            if !Self::check_expected_type_for_expr(&expected_ty, &arg_ty, arg) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento {} da chamada dinâmica '{}.{}': esperado '{}', encontrado '{}'",
                        index + 1,
                        trait_name,
                        method_name,
                        Self::type_key(&expected_ty),
                        Self::type_key(&arg_ty)
                    ),
                    span: arg.span,
                });
            }

            Self::validate_int_literal_range(&expected_ty, arg)?;
        }

        match method_ret_type {
            Some(ret_type) => self
                .resolve_type_or_error(&ret_type)
                .map(|resolved| resolved.with_span(expr_span)),
            None => Ok(Type::Nulo(expr_span)),
        }
    }

    fn resolve_impl_method(
        &self,
        receiver_ty: &Type,
        method_name: &str,
        span: Span,
    ) -> Result<String, PinkerError> {
        let direct_key = Self::type_key(receiver_ty);
        let resolved_key = self.resolved_type_identity(receiver_ty)?;
        let candidates = self
            .method_index
            .get(&(resolved_key, method_name.to_string()))
            .cloned()
            .unwrap_or_default();
        // Esta fase só constrói candidatos a partir do seu próprio índice e
        // nomeia a relação de cada um. Quem alcança, quem precede e quem vence
        // é `method_dispatch`, a autoridade única — o lowering resolve a mesma
        // chamada pela mesma regra, e por isso não pode discordar daqui.
        let candidates = candidates.into_iter().map(|function_name| {
            let relation = self
                .impl_methods
                .iter()
                .find(|meta| meta.function_name == function_name)
                .map(|meta| DispatchRelation {
                    fonte_da_relacao: self
                        .fonte_da_relacao(&meta.identity.trait_name, &meta.identity.target),
                });
            DispatchCandidate {
                function_name,
                relation,
            }
        });

        match method_dispatch::select_impl_method(&self.traits_visiveis_por_fonte, span, candidates)
        {
            MethodSelection::Winner(function_name) => Ok(function_name),
            MethodSelection::NoMatch => Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' não implementado para tipo '{}'",
                    method_name, direct_key
                ),
                span,
            }),
            MethodSelection::Ambiguous => Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' para tipo '{}' é ambíguo; use 'Trato.{}(valor, ...)'",
                    method_name, direct_key, method_name
                ),
                span,
            }),
        }
    }

    fn resolve_qualified_impl_method(
        &self,
        trait_name: &str,
        receiver_ty: &Type,
        method_name: &str,
        span: Span,
    ) -> Result<String, PinkerError> {
        let direct_key = Self::type_key(receiver_ty);
        let resolved_key = self.resolved_type_identity(receiver_ty)?;
        // #647/U-03A: a correspondência é de `method_identity`, a mesma que o
        // lowering consulta. Aqui só sobram o adaptador de representação — o
        // alvo vira identidade resolvida antes da consulta — e a mensagem, que
        // é da fase.
        let function_name = match method_identity::resolve_qualified_impl_method(
            self.impl_methods
                .iter()
                .map(|meta| (&meta.identity, meta.function_name.as_str())),
            trait_name,
            &resolved_key,
            method_name,
        ) {
            QualifiedMethodResolution::Resolved(function_name) => function_name,
            QualifiedMethodResolution::NoMatch => {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "método '{}.{}' não implementado para tipo '{}'",
                        trait_name, method_name, direct_key
                    ),
                    span,
                })
            }
        };
        // #649 — qualificar não é bypass da política modular. A identidade
        // exata responde QUAL relação; o alcance responde se ESTE contexto pode
        // usá-la, e as duas perguntas continuam com donos distintos.
        if !self.relacao_alcanca(trait_name, &resolved_key, span) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "impl de '{}' para tipo '{}' não é alcançável desta unidade; \
                     importe a unidade que declara essa implementação",
                    trait_name, direct_key
                ),
                span,
            });
        }
        Ok(function_name)
    }

    fn generic_map_monomorphic_callee(map_ty: &Type, name: &str) -> Option<&'static str> {
        match (map_ty, name) {
            (Type::MapVersoBombom(_), "mapa_definir") => Some("mapa_verso_bombom_definir"),
            (Type::MapVersoBombom(_), "mapa_obter") => Some("mapa_verso_bombom_obter"),
            (Type::MapVersoBombom(_), "mapa_tem") => Some("mapa_verso_bombom_tem"),
            (Type::MapVersoBombom(_), "mapa_tamanho") => Some("mapa_verso_bombom_tamanho"),
            (Type::MapVersoBombom(_), "mapa_remover") => Some("mapa_verso_bombom_remover"),
            (Type::MapVersoVerso(_), "mapa_definir") => Some("mapa_verso_verso_definir"),
            (Type::MapVersoVerso(_), "mapa_obter") => Some("mapa_verso_verso_obter"),
            (Type::MapVersoVerso(_), "mapa_tem") => Some("mapa_verso_verso_tem"),
            (Type::MapVersoVerso(_), "mapa_tamanho") => Some("mapa_verso_verso_tamanho"),
            (Type::MapVersoVerso(_), "mapa_remover") => Some("mapa_verso_verso_remover"),
            (Type::MapBombomBombom(_), "mapa_definir") => Some("mapa_bombom_bombom_definir"),
            (Type::MapBombomBombom(_), "mapa_obter") => Some("mapa_bombom_bombom_obter"),
            (Type::MapBombomBombom(_), "mapa_tem") => Some("mapa_bombom_bombom_tem"),
            (Type::MapBombomBombom(_), "mapa_tamanho") => Some("mapa_bombom_bombom_tamanho"),
            (Type::MapBombomBombom(_), "mapa_remover") => Some("mapa_bombom_bombom_remover"),
            (Type::MapBombomVerso(_), "mapa_definir") => Some("mapa_bombom_verso_definir"),
            (Type::MapBombomVerso(_), "mapa_obter") => Some("mapa_bombom_verso_obter"),
            (Type::MapBombomVerso(_), "mapa_tem") => Some("mapa_bombom_verso_tem"),
            (Type::MapBombomVerso(_), "mapa_tamanho") => Some("mapa_bombom_verso_tamanho"),
            (Type::MapBombomVerso(_), "mapa_remover") => Some("mapa_bombom_verso_remover"),
            _ => None,
        }
    }

    fn check_named_function_call(
        &mut self,
        expr_span: Span,
        callee_span: Span,
        name: &str,
        args: &[&Expr],
    ) -> Result<Type, PinkerError> {
        // MODULE_IMPORTER_NON_INTERFERENCE, última fronteira: a busca por
        // função de usuário acontece depois do despacho de intrínsecas, então
        // chegar aqui com grafia crua vinda de um módulo significa que o
        // builtin não atendeu e a única candidata restante é da raiz.
        if self.grafia_crua_de_modulo(callee_span, name) && self.funcs.contains_key(name) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "função '{}' não declarada neste ambiente: ela é declarada na raiz, e o módulo não a importou",
                    name
                ),
                span: callee_span,
            });
        }
        let Some(function) = self.funcs.get(name).cloned() else {
            return Err(PinkerError::Semantic {
                msg: format!("função '{}' não declarada", name),
                span: callee_span,
            });
        };

        if args.len() != function.params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                    name,
                    function.params.len(),
                    args.len()
                ),
                span: expr_span,
            });
        }

        for (index, (arg, param)) in args.iter().zip(function.params.iter()).enumerate() {
            let arg_ty = self.check_value_expr(
                arg,
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            let expected_param_ty = self.resolve_type_or_error(&param.ty)?;
            if !Self::check_expected_type_for_expr(&expected_param_ty, &arg_ty, arg) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento {} da chamada '{}': esperado '{}', encontrado '{}'",
                        index + 1,
                        name,
                        Self::type_key(&expected_param_ty),
                        Self::type_key(&arg_ty)
                    ),
                    span: arg.span,
                });
            }
            Self::validate_int_literal_range(&expected_param_ty, arg)?;
        }

        Ok(self.function_result_type(&function, expr_span))
    }

    pub(super) fn check_call_expr(
        &mut self,
        expr_span: Span,
        callee: &Expr,
        args: &[Expr],
    ) -> Result<Type, PinkerError> {
        // Construção de variante de leque: `Leque.Variante(carga)`.
        if let ExprKind::FieldAccess { base, field } = &callee.kind {
            if let ExprKind::Ident(base_name) = &base.kind {
                if let Some(enum_name) = self.resolve_enum_base_name(base_name) {
                    let enum_decl = self.enums.get(&enum_name).expect("leque resolvido existe");
                    let Some(variant) = enum_decl
                        .variants
                        .iter()
                        .find(|variant| variant.name == *field)
                        .cloned()
                    else {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "variante '{}' não existe no leque '{}'",
                                field, base_name
                            ),
                            span: expr_span,
                        });
                    };
                    if variant.payloads.is_empty() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "variante '{}' não carrega valor; use '{}.{}' sem parênteses",
                                field, enum_name, field
                            ),
                            span: expr_span,
                        });
                    }
                    if args.len() != variant.payloads.len() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "construção de '{}.{}' exige {} argumento(s) de carga, recebido {}",
                                enum_name,
                                field,
                                variant.payloads.len(),
                                args.len()
                            ),
                            span: expr_span,
                        });
                    }
                    for (index, payload_ty) in variant.payloads.iter().enumerate() {
                        let expected = self.resolve_type_or_error(payload_ty)?;
                        let arg_ty = self.check_value_expr(
                            &args[index],
                            "resultado de função sem retorno não pode ser carga de variante",
                        )?;
                        if !Self::check_expected_type_for_expr(&expected, &arg_ty, &args[index]) {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "carga {} inválida para '{}.{}': esperado '{}', encontrado '{}'",
                                    index + 1,
                                    enum_name,
                                    field,
                                    // Nome fiel: a mensagem existe para
                                    // distinguir `lista<Cor>` de `lista<Token>`,
                                    // que compartilham a categoria operacional.
                                    expected.display_name(),
                                    arg_ty.display_name()
                                ),
                                span: args[index].span,
                            });
                        }
                    }
                    return Ok(Type::Enum {
                        name: enum_name,
                        span: expr_span,
                    });
                }
                // Parte G: `familia.membro(...)` sem o `trazer`, em posição de
                // chamada. Mesma regra do acesso a campo — a dica só sai
                // depois de o nome se provar órfão.
                if let Some(erro) = self.dica_de_familia_nao_importada(base_name, field, expr_span)
                {
                    return Err(erro);
                }
                if self.traits.contains_key(base_name) {
                    if args.is_empty() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "chamada qualificada '{}.{}' exige receiver como primeiro argumento",
                                base_name, field
                            ),
                            span: expr_span,
                        });
                    }
                    let receiver_ty = self.check_value_expr(
                        &args[0],
                        "resultado de função sem retorno não pode ser receiver de método",
                    )?;
                    if Self::trait_object_name(&receiver_ty) == Some(base_name.as_str()) {
                        return self.check_trait_object_method_call(
                            expr_span,
                            callee.span,
                            base_name,
                            field,
                            &args[1..],
                        );
                    }

                    let function_name = self.resolve_qualified_impl_method(
                        base_name,
                        &receiver_ty,
                        field,
                        callee.span,
                    )?;
                    let qualified_args: Vec<&Expr> = args.iter().collect();
                    return self.check_named_function_call(
                        expr_span,
                        callee.span,
                        &function_name,
                        &qualified_args,
                    );
                }
            }
            let receiver_ty = self.check_value_expr(
                base,
                "resultado de função sem retorno não pode ser receiver de método",
            )?;
            if let Some(trait_name) = Self::trait_object_name(&receiver_ty).map(str::to_string) {
                return self.check_trait_object_method_call(
                    expr_span,
                    callee.span,
                    &trait_name,
                    field,
                    args,
                );
            }

            let function_name = match self.resolve_impl_method(&receiver_ty, field, callee.span) {
                Ok(function_name) => function_name,
                Err(_) if self.funcs.contains_key(field) => field.clone(),
                Err(err) => return Err(err),
            };
            let mut method_args = Vec::with_capacity(args.len() + 1);
            method_args.push(base.as_ref());
            method_args.extend(args.iter());
            return self.check_named_function_call(
                expr_span,
                callee.span,
                &function_name,
                &method_args,
            );
        }

        // #532 — CANONICALIZATION_BOUNDARY do lado do consumidor.
        //
        // `nome_fonte` é o texto que o usuário escreveu (ou o nome sintético que
        // o compilador materializou); `identidade_do_callee` é a decisão que a
        // resolução já tomou. As duas coisas eram uma só, e por isso a cadeia
        // de intrínsecas abaixo respondia por uma função do usuário homônima.
        let identidade_do_callee = match &callee.kind {
            ExprKind::Intrinsic(identity) => {
                crate::intrinsics::identity::CalleeIdentity::Intrinsic(*identity)
            }
            ExprKind::Ident(name) => {
                crate::intrinsics::identity::callee_identity_de_ident(name.as_str())
            }
            _ => crate::intrinsics::identity::CalleeIdentity::User,
        };
        let grafia_do_callee = match &callee.kind {
            ExprKind::Intrinsic(identity) => Some(identity.canonical_public_spelling().to_string()),
            ExprKind::Ident(name) => Some(name.clone()),
            _ => None,
        };
        let Some(name) = grafia_do_callee.as_ref() else {
            let callee_ty = self.check_value_expr(
                callee,
                "resultado sem retorno não pode ocupar a posição de chamada",
            )?;
            let Type::Pointer { ref base, .. } = callee_ty else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "expressão em posição de chamada não é chamável (tipo '{}')",
                        Self::type_key(&callee_ty)
                    ),
                    span: callee.span,
                });
            };
            let Type::Function { params, ret, .. } = base.as_ref() else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "expressão em posição de chamada não é ponteiro cru de função (tipo '{}')",
                        Self::type_key(&callee_ty)
                    ),
                    span: callee.span,
                });
            };
            if args.len() != params.len() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada por expressão de ponteiro cru com aridade inválida: esperado {}, recebido {}",
                        params.len(),
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let params = params.clone();
            let ret = ret.as_ref().clone();
            for (index, (arg, expected)) in args.iter().zip(params.iter()).enumerate() {
                let arg_ty = self.check_value_expr(
                    arg,
                    "resultado sem retorno não pode ser argumento de ponteiro cru",
                )?;
                let expected_resolved = self.resolve_type_or_error(expected)?;
                if !Self::check_expected_type_for_expr(&expected_resolved, &arg_ty, arg) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento {} da chamada por expressão de ponteiro cru: esperado '{}', encontrado '{}'",
                            index + 1,
                            Self::type_key(&expected_resolved),
                            Self::type_key(&arg_ty)
                        ),
                        span: arg.span,
                    });
                }
                Self::validate_int_literal_range(&expected_resolved, arg)?;
            }
            return self.resolve_type_or_error(&ret);
        };

        // Fase 242: variável local (parâmetro ou `nova`) tem precedência
        // sobre função top-level homônima em posição de chamada — chamada
        // indireta real, sem depender de resolução estática do nome
        // concreto no parse (ao contrário da especialização da Fase 239).
        if let Some(local_ty) = self.resolve_local_var_type(name) {
            let (params, ret, raw) = match &local_ty {
                Type::Function { params, ret, .. } => (params, ret.as_ref(), false),
                Type::Pointer { base, .. } => match base.as_ref() {
                    Type::Function { params, ret, .. } => (params, ret.as_ref(), true),
                    _ => {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "'{}' não é ponteiro de função chamável (tipo '{}')",
                                name,
                                Self::type_key(&local_ty)
                            ),
                            span: callee.span,
                        });
                    }
                },
                _ => {
                    return Err(PinkerError::Semantic {
                        msg: format!("'{}' não é chamável (tipo '{}')", name, local_ty.name()),
                        span: callee.span,
                    });
                }
            };
            if args.len() != params.len() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "{} de '{}' com aridade inválida: esperado {}, recebido {}",
                        if raw {
                            "chamada por ponteiro cru"
                        } else {
                            "chamada indireta"
                        },
                        name,
                        params.len(),
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let params = params.clone();
            let ret = ret.clone();
            for (index, (arg, expected)) in args.iter().zip(params.iter()).enumerate() {
                let arg_ty = self.check_value_expr(
                    arg,
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                let expected_resolved = self.resolve_type_or_error(expected)?;
                if !Self::check_expected_type_for_expr(&expected_resolved, &arg_ty, arg) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento {} da {} de '{}': esperado '{}', encontrado '{}'",
                            index + 1,
                            if raw {
                                "chamada por ponteiro cru"
                            } else {
                                "chamada indireta"
                            },
                            name,
                            Self::type_key(&expected_resolved),
                            Self::type_key(&arg_ty)
                        ),
                        span: arg.span,
                    });
                }
                Self::validate_int_literal_range(&expected_resolved, arg)?;
            }
            return self.resolve_type_or_error(&ret);
        }

        // #532 — daqui para baixo começa a cadeia de intrínsecas, e ela só é
        // atravessada por um callee cuja IDENTIDADE diz que ele é builtin.
        //
        // ```text
        // CALL_IS_INTRINSIC <- RESOLVED_IDENTITY, NOT SPELLING
        // ```
        //
        // Antes, a cadeia era atravessada por qualquer chamada, e a primeira
        // comparação textual que casasse decidia. Era isso que obrigava a
        // reservar as grafias canônicas contra declaração do usuário: sem a
        // reserva, `carinho tamanho_verso(...)` seria aceito e depois sombreado
        // aqui em silêncio. Com a decisão vindo da identidade, o callee de
        // usuário vai direto para a resolução de função de usuário.
        if identidade_do_callee.is_user() {
            let arg_refs: Vec<&Expr> = args.iter().collect();
            return self.check_named_function_call(expr_span, callee.span, name, &arg_refs);
        }

        // Fase 246: superfície pública de memória explícita. O tamanho é
        // sempre expresso em bytes (`u64`) e o ponteiro devolvido é `seta<u8>`.
        if name == "alocar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'alocar' exige exatamente 1 argumento de tamanho em bytes, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let expected = Type::U64(args[0].span);
            let actual = self.check_value_expr(
                &args[0],
                "resultado sem retorno não pode ser tamanho de alocação",
            )?;
            if !Self::check_expected_type_for_expr(&expected, &actual, &args[0]) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'alocar' exige tamanho 'u64' em bytes; encontrado '{}'",
                        Self::type_key(&actual)
                    ),
                    span: args[0].span,
                });
            }
            Self::validate_int_literal_range(&expected, &args[0])?;
            return Ok(Type::Pointer {
                base: Box::new(Type::U8(expr_span)),
                is_volatile: false,
                span: expr_span,
            });
        }
        if name == "liberar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'liberar' exige exatamente 1 ponteiro-base, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let actual =
                self.check_value_expr(&args[0], "resultado sem retorno não pode ser liberado")?;
            let expected = Type::Pointer {
                base: Box::new(Type::U8(args[0].span)),
                is_volatile: false,
                span: args[0].span,
            };
            if !Self::check_type_match(&expected, &actual) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'liberar' exige ponteiro-base 'seta<u8>'; encontrado '{}'",
                        Self::type_key(&actual)
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Nulo(expr_span));
        }

        if matches!(
            name.as_str(),
            "mapa_definir" | "mapa_obter" | "mapa_tem" | "mapa_tamanho" | "mapa_remover"
        ) {
            let Some(first_arg) = args.first() else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado ao menos 1 argumento",
                        name
                    ),
                    span: expr_span,
                });
            };
            let map_ty = self.check_value_expr(
                first_arg,
                "resultado de função sem retorno não pode ser usado como mapa",
            )?;
            if let Some(mono_name) = Self::generic_map_monomorphic_callee(&map_ty, name) {
                // #532: a monomorfização troca a grafia DENTRO da identidade
                // intrínseca. Reemitir um `Ident` aqui devolveria a chamada ao
                // namespace do usuário, e ela voltaria a poder ser capturada
                // por uma função homônima.
                let mono_callee = Expr {
                    kind: ExprKind::Intrinsic(
                        crate::intrinsics::identity::intrinsic_from_public_spelling(mono_name)
                            .expect("forma monomórfica de mapa é grafia pública registrada"),
                    ),
                    span: callee.span,
                };
                return self.check_call_expr(expr_span, &mono_callee, args);
            }
            if let Type::Map { key, value, .. } = &map_ty {
                let expected_arity = match name.as_str() {
                    "mapa_definir" => 3,
                    "mapa_tamanho" => 1,
                    _ => 2,
                };
                if args.len() != expected_arity {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                            name,
                            expected_arity,
                            args.len()
                        ),
                        span: expr_span,
                    });
                }
                if expected_arity >= 2 {
                    let actual_key = self.check_value_expr(
                        &args[1],
                        "resultado sem retorno não pode ser usado como chave de mapa",
                    )?;
                    if !Self::check_type_match(key, &actual_key) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "chave incompatível em '{}': esperado '{}', encontrado '{}'",
                                name,
                                Self::type_key(key),
                                Self::type_key(&actual_key)
                            ),
                            span: args[1].span,
                        });
                    }
                }
                if name == "mapa_definir" {
                    let actual_value = self.check_value_expr(
                        &args[2],
                        "resultado sem retorno não pode ser armazenado em mapa",
                    )?;
                    if !Self::check_type_match(value, &actual_value) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "valor incompatível em 'mapa_definir': esperado '{}', encontrado '{}'",
                                Self::type_key(value),
                                Self::type_key(&actual_value)
                            ),
                            span: args[2].span,
                        });
                    }
                }
                return Ok(match name.as_str() {
                    "mapa_obter" => value.with_span(expr_span),
                    "mapa_tem" => Type::Logica(expr_span),
                    "mapa_tamanho" => Type::Bombom(expr_span),
                    "mapa_definir" | "mapa_remover" => Type::Nulo(expr_span),
                    _ => unreachable!(),
                });
            }
            return Err(PinkerError::Semantic {
                msg: format!(
                    "operação genérica '{}' exige mapa como primeiro argumento; encontrado '{}'",
                    name,
                    map_ty.name()
                ),
                span: first_arg.span,
            });
        }

        if name == "__pinker_internal_mapa_iterador_criar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let map_ty = self.check_value_expr(
                &args[0],
                "resultado sem retorno não pode ser iterado como mapa",
            )?;
            if !matches!(map_ty, Type::Map { .. }) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "iterador interno exige mapa genérico; encontrado '{}'",
                        Self::type_key(&map_ty)
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }

        if matches!(
            name.as_str(),
            "__pinker_internal_mapa_iterador_proxima_chave_bombom"
                | "__pinker_internal_mapa_iterador_proxima_chave_verso"
        ) {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "avanço de iterador interno de mapa exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let cursor_ty = self.check_value_expr(
                &args[0],
                "resultado sem retorno não pode ser cursor de mapa",
            )?;
            if !matches!(cursor_ty, Type::Bombom(_)) {
                return Err(PinkerError::Semantic {
                    msg: "cursor interno de mapa exige 'bombom'".to_string(),
                    span: args[0].span,
                });
            }
            return Ok(if name.ends_with("_verso") {
                Type::Verso(expr_span)
            } else {
                Type::Bombom(expr_span)
            });
        }

        // As operações de tag e extração de união **não** são chamadas da
        // linguagem: são nós tipados da IR (`ValueIR::UnionTag` e
        // `ValueIR::UnionExtract`) criados pelo lowering a partir de
        // `Stmt::UnionMatch`. Não há intrínseca de união chamável aqui, e o
        // namespace `__pinker_internal_` permanece recusado à fonte.

        // Intrínsecas internas do desugaring de `encaixe` (Fases 209–210).
        if name == "__pinker_internal_leque_tag" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' exige 1 argumento (valor de leque)",
                        name
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser inspecionado por encaixe",
            )?;
            let Type::Enum {
                name: enum_name, ..
            } = &arg_ty
            else {
                return Err(PinkerError::Semantic {
                    msg: format!("intrínseca interna '{}' exige valor de leque", name),
                    span: args[0].span,
                });
            };
            if !self.enum_has_payload(enum_name) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' exige leque com carga; '{}' não tem variantes com carga",
                        name, enum_name
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if crate::enum_payload::is_carga_intrinsic(name) {
            if args.len() != 3 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' exige 3 argumentos (leque, tag, índice)",
                        name
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser inspecionado por encaixe",
            )?;
            let Type::Enum {
                name: enum_name, ..
            } = &arg_ty
            else {
                return Err(PinkerError::Semantic {
                    msg: format!("intrínseca interna '{}' exige valor de leque", name),
                    span: args[0].span,
                });
            };
            let (ExprKind::IntLit(tag), ExprKind::IntLit(index)) = (&args[1].kind, &args[2].kind)
            else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' exige tag e índice literais (uso interno do encaixe)",
                        name
                    ),
                    span: expr_span,
                });
            };
            let enum_name = enum_name.clone();
            let payload_ty = self
                .enums
                .get(&enum_name)
                .and_then(|decl| decl.variants.get(*tag as usize))
                .and_then(|variant| variant.payloads.get(*index as usize))
                .cloned();
            let Some(payload_ty) = payload_ty else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' referencia carga inexistente no leque '{}'",
                        name, enum_name
                    ),
                    span: expr_span,
                });
            };
            // O helper usado tem de ser exatamente o que a autoridade de
            // classificação escolhe para esta carga. Isso impede que uma
            // extração `lista<verso>` seja encaminhada pelo caminho de `verso`
            // — ou o contrário — mesmo que ambos caibam numa palavra.
            let shape = self.classify_enum_payload(&payload_ty).map_err(|rejection| {
                PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' usada com carga sem classificação no leque '{}'; {}",
                        name,
                        enum_name,
                        rejection.message()
                    ),
                    span: expr_span,
                }
            })?;
            if shape.carga_intrinsic() != name {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' usada com carga de classe '{}' no leque '{}'; o helper correto é '{}'",
                        name,
                        shape.class.name(),
                        enum_name,
                        shape.carga_intrinsic()
                    ),
                    span: expr_span,
                });
            }
            return Ok(shape.resolved.with_span(expr_span));
        }

        // Intrínsecas genéricas de lista (Fase 211): tipam sobre qualquer
        // lista (`lista<bombom>`, `lista<verso>`, `lista<Leque>`), com o tipo
        // do elemento derivado do primeiro argumento.
        if name == "lista_criar" {
            return Err(PinkerError::Semantic {
                msg: "'lista_criar()' só pode aparecer como inicialização de 'nova' com anotação de tipo de lista nesta fase"
                    .to_string(),
                span: expr_span,
            });
        }
        if matches!(
            name.as_str(),
            "lista_tamanho"
                | "lista_obter"
                | "lista_anexar"
                | "lista_definir"
                | "lista_tirar_ultimo"
                | "lista_inserir"
        ) {
            let expected_arity: usize = match name.as_str() {
                "lista_tamanho" | "lista_tirar_ultimo" => 1,
                "lista_obter" | "lista_anexar" => 2,
                _ => 3,
            };
            if args.len() != expected_arity {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                        name,
                        expected_arity,
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let list_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como lista",
            )?;
            let Some(element_ty) = Self::list_element_type(&list_ty, expr_span) else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'{}' exige lista no argumento 1, encontrado '{}'",
                        name,
                        list_ty.name()
                    ),
                    span: args[0].span,
                });
            };
            let check_index = |checker: &mut Self, index_arg: &Expr| -> Result<(), PinkerError> {
                let index_ty = checker.check_value_expr(
                    index_arg,
                    "resultado de função sem retorno não pode ser índice",
                )?;
                if !matches!(index_ty, Type::Bombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!("'{}' exige índice 'bombom'", name),
                        span: index_arg.span,
                    });
                }
                Ok(())
            };
            let check_element = |checker: &mut Self, value_arg: &Expr| -> Result<(), PinkerError> {
                let value_ty = checker.check_value_expr(
                    value_arg,
                    "resultado de função sem retorno não pode ser elemento de lista",
                )?;
                if !Self::check_expected_type_for_expr(&element_ty, &value_ty, value_arg) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "'{}' exige elemento '{}', encontrado '{}'",
                            name,
                            element_ty.name(),
                            value_ty.name()
                        ),
                        span: value_arg.span,
                    });
                }
                Ok(())
            };
            return match name.as_str() {
                "lista_tamanho" => Ok(Type::Bombom(expr_span)),
                "lista_tirar_ultimo" => Ok(element_ty),
                "lista_obter" => {
                    check_index(self, &args[1])?;
                    Ok(element_ty)
                }
                "lista_anexar" => {
                    check_element(self, &args[1])?;
                    Ok(Type::Nulo(expr_span))
                }
                "lista_definir" | "lista_inserir" => {
                    check_index(self, &args[1])?;
                    check_element(self, &args[2])?;
                    Ok(Type::Nulo(expr_span))
                }
                _ => unreachable!(),
            };
        }

        // #442/C1 — o contrato declarativo da grafia histórica vem do registry.
        //
        // Estes cento e dez blocos eram cento e dez cópias do mesmo formato:
        // aridade exata, um tipo por argumento, tipo de retorno. O texto dos
        // diagnósticos é idêntico ao que cada bloco montava — a grafia usada é
        // a que o call site escreveu, então alias histórico continua se
        // diagnosticando pelo próprio nome.
        //
        // As grafias com contrato próprio — aridade variável, forma genérica
        // ainda não monomorfizada ou restrição que não cabe em
        // `(params, ret)` — seguem abaixo, marcadas no registry como
        // `SemanticContract::PhaseSpecific`.
        if let Some(contrato) = crate::intrinsics::registry::entrada(name) {
            if contrato.semantic == crate::intrinsics::registry::SemanticContract::Declared {
                let (ret, params) = contrato
                    .assinatura_ir()
                    .expect("contrato declarado tem assinatura no registry");
                if args.len() != params.len() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                            name,
                            params.len(),
                            args.len()
                        ),
                        span: expr_span,
                    });
                }
                for (index, (arg, esperado)) in args.iter().zip(params).enumerate() {
                    let arg_ty = self.check_value_expr(
                        arg,
                        "resultado de função sem retorno não pode ser usado como argumento",
                    )?;
                    let esperado = Self::tipo_de_intrinseca(*esperado, arg.span);
                    if std::mem::discriminant(&arg_ty) != std::mem::discriminant(&esperado) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "tipo inválido no argumento {} da chamada '{}': esperado '{}', encontrado '{}'",
                                index + 1,
                                name,
                                esperado.name(),
                                arg_ty.name()
                            ),
                            span: arg.span,
                        });
                    }
                }
                return Ok(Self::tipo_de_intrinseca(ret, expr_span));
            }
        }
        if name == "afirmar" {
            if args.is_empty() || args.len() > 2 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'afirmar' com aridade inválida: esperado 1 ou 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let cond_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(cond_ty, Type::Logica(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'afirmar': esperado 'logica', encontrado '{}'",
                        cond_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            if args.len() == 2 {
                let msg_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(msg_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada 'afirmar': esperado 'verso', encontrado '{}'",
                            msg_ty.name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(Type::Nulo(expr_span));
        }
        // Superfícies falíveis: assinatura e retorno vêm da autoridade única.
        if let Some(superficie) = crate::falha_operacional::superficie(name) {
            // Parte B1: esta chamada produz uma tag cujo significado vem da
            // taxonomia do leque materializado. Aqui o programa já está
            // completo — imports resolvidos, genéricos monomorfizados —, então é
            // o ponto onde a pergunta pode ser respondida sobre o artefato
            // inteiro, e não sobre uma unidade de compilação. A condição
            // continua sendo a mesma da conjunção do parser: só é verificada
            // porque o programa realmente produz o valor.
            self.check_runtime_result_identity(superficie, expr_span)?;
            if args.len() != superficie.aridade() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                        name,
                        superficie.aridade(),
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            for (index, (arg, esperado)) in
                args.iter().zip(superficie.argumentos.iter()).enumerate()
            {
                let arg_ty = self.check_value_expr(
                    arg,
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !esperado.aceita(&arg_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento {} da chamada '{}': esperado '{}', encontrado '{}'",
                            index + 1,
                            name,
                            esperado.nome_para_diagnostico(),
                            arg_ty.display_name()
                        ),
                        span: arg.span,
                    });
                }
            }
            return Ok(superficie.tipo_de_retorno(expr_span));
        }
        // Parte E1 — acessores da árvore JSON.
        //
        // O primeiro argumento é sempre `ValorJson`; a aridade e o tipo do
        // segundo derivam da intrínseca. `json_lista_obter` e
        // `json_objeto_obter` devolvem `ValorJson`, que é o que torna o nesting
        // atravessável sem superfície nova por formato.
        if crate::valor_json::e_acessor(name) {
            use crate::valor_json::intrinsecas as ji;
            let segundo: Option<Type> = match name.as_str() {
                ji::LISTA_OBTER => Some(Type::Bombom(expr_span)),
                ji::OBJETO_TEM | ji::OBJETO_OBTER => Some(Type::Verso(expr_span)),
                _ => None,
            };
            let esperado = 1 + usize::from(segundo.is_some());
            if args.len() != esperado {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                        name,
                        esperado,
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(
                arg_ty,
                Type::OpaqueHandle {
                    name: ref handle_name,
                    ..
                } if handle_name == crate::valor_json::TIPO_VALOR_JSON
            ) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada '{}': esperado '{}', encontrado '{}'",
                        name,
                        crate::valor_json::TIPO_VALOR_JSON,
                        arg_ty.display_name()
                    ),
                    span: args[0].span,
                });
            }
            if let Some(esperado_2) = segundo {
                let arg2_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                let compativel = match esperado_2 {
                    Type::Bombom(_) => matches!(arg2_ty, Type::Bombom(_)),
                    Type::Verso(_) => matches!(arg2_ty, Type::Verso(_)),
                    _ => false,
                };
                if !compativel {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada '{}': esperado '{}', encontrado '{}'",
                            name,
                            esperado_2.name(),
                            arg2_ty.display_name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(match name.as_str() {
                ji::EMITIR | ji::VERSO => Type::Verso(expr_span),
                ji::TIPO => Type::Enum {
                    name: crate::valor_json::LEQUE_TIPO_JSON.to_string(),
                    span: expr_span,
                },
                ji::NUMERO => Type::I64(expr_span),
                ji::LOGICA | ji::OBJETO_TEM => Type::Logica(expr_span),
                ji::LISTA_TAMANHO | ji::OBJETO_TAMANHO => Type::Bombom(expr_span),
                ji::LISTA_OBTER | ji::OBJETO_OBTER => Type::OpaqueHandle {
                    name: crate::valor_json::TIPO_VALOR_JSON.to_string(),
                    span: expr_span,
                },
                ji::OBJETO_CHAVES => Type::ListVerso(expr_span),
                _ => unreachable!("acessor JSON sem tipo de retorno"),
            });
        }
        if crate::saida_processo::e_acessor(name) {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado 1, recebido {}",
                        name,
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(
                arg_ty,
                Type::OpaqueHandle {
                    name: ref handle_name,
                    ..
                } if handle_name == crate::saida_processo::TIPO_SAIDA_PROCESSO
            ) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada '{}': esperado 'SaidaProcesso', encontrado '{}'",
                        name,
                        arg_ty.display_name()
                    ),
                    span: args[0].span,
                });
            }
            return Ok(match name.as_str() {
                crate::saida_processo::ACESSOR_CODIGO => Type::Bombom(expr_span),
                crate::saida_processo::ACESSOR_SAIDA | crate::saida_processo::ACESSOR_ERRO => {
                    Type::Verso(expr_span)
                }
                _ => unreachable!(),
            });
        }
        // Parte E2: `sha256_verso(verso) -> verso`. Aridade e tipo errados são
        // INVALID_PROGRAM_USE — erro de compilação, nunca `Resultado`.
        if crate::sha256::e_acessor(name) {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'sha256_verso' com aridade inválida: esperado 1, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(arg_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'sha256_verso': esperado 'verso', encontrado '{}'",
                        arg_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Verso(expr_span));
        }
        // Fase 140 — buscar_verso(texto, padrao) -> bombom
        // Fase 137 — dividir_verso_em(texto, sep, indice) -> verso
        // Fase 137 — dividir_verso_contar(texto, sep) -> bombom
        // Fase 138 — substituir_verso(texto, de, para) -> verso

        // Fase 139 — juntar_verso_com(a, sep, b) -> verso

        // Fase 157 — formatar_verso(modelo, a[, b, ...]) -> verso
        if name == "formatar_verso" {
            if args.len() < 2 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'formatar_verso' com aridade inválida: esperado pelo menos 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let modelo_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(modelo_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'formatar_verso': esperado 'verso', encontrado '{}'",
                        modelo_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            for (idx, arg) in args.iter().enumerate().skip(1) {
                let arg_ty = self.check_value_expr(
                    arg,
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(arg_ty, Type::Bombom(_) | Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento {} da chamada 'formatar_verso': esperado 'bombom' ou 'verso', encontrado '{}'",
                            idx + 1,
                            arg_ty.name()
                        ),
                        span: arg.span,
                    });
                }
            }
            return Ok(Type::Verso(expr_span));
        }

        if name == "__ternario" {
            if args.len() != 3 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "expressão ternária requer exatamente 3 argumentos (condição, valor_verdade, valor_falso), recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let cond_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como condição ternária",
            )?;
            if !matches!(cond_ty, Type::Logica(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "condição da expressão ternária deve ser 'logica', encontrado '{}'",
                        cond_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            let then_ty = self.check_value_expr(
                &args[1],
                "resultado de função sem retorno não pode ser usado em expressão ternária",
            )?;
            let else_ty = self.check_value_expr(
                &args[2],
                "resultado de função sem retorno não pode ser usado em expressão ternária",
            )?;
            if then_ty != else_ty {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "ramos da expressão ternária devem ter o mesmo tipo: '{}' vs '{}'",
                        then_ty.name(),
                        else_ty.name()
                    ),
                    span: expr_span,
                });
            }
            return Ok(then_ty.with_span(expr_span));
        }

        // Fase 158 — ler_linha_csv_bombom(linha, sep) -> lista<bombom>

        // Fase 158 — emitir_linha_csv_bombom(itens, sep) -> verso

        // Fase 159 — ler_json_plano_bombom(json) -> mapa<verso,bombom>

        // Fase 159 — emitir_json_plano_bombom(mapa) -> verso

        // Fase 160 — tempo_unix() -> bombom

        // Fase 160 — formatar_tempo_unix(ts) -> verso

        // Fase 168 — executar_processo(comando[, argv1]) -> bombom
        if name == "executar_processo" {
            if !(1..=2).contains(&args.len()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'executar_processo' com aridade inválida: esperado 1 ou 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let command_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(command_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'executar_processo': esperado 'verso', encontrado '{}'",
                        command_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            if args.len() == 2 {
                let argv1_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(argv1_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada 'executar_processo': esperado 'verso', encontrado '{}'",
                            argv1_ty.name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(Type::Bombom(expr_span));
        }

        // Fase 177 — executar_com_entrada(comando, entrada[, argv1]) -> bombom
        if name == "executar_com_entrada" {
            if !(2..=3).contains(&args.len()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'executar_com_entrada' com aridade inválida: esperado 2 ou 3, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let command_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(command_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'executar_com_entrada': esperado 'verso', encontrado '{}'",
                        command_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            let input_ty = self.check_value_expr(
                &args[1],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(input_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 2 da chamada 'executar_com_entrada': esperado 'verso', encontrado '{}'",
                        input_ty.name()
                    ),
                    span: args[1].span,
                });
            }
            if args.len() == 3 {
                let argv1_ty = self.check_value_expr(
                    &args[2],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(argv1_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 3 da chamada 'executar_com_entrada': esperado 'verso', encontrado '{}'",
                            argv1_ty.name()
                        ),
                        span: args[2].span,
                    });
                }
            }
            return Ok(Type::Bombom(expr_span));
        }

        // Fase 166 — pipeline_minimo(produtor, consumidor) -> bombom

        // Fase 169 — capturar_stdout(comando[, argv1]) -> verso
        if name == "capturar_stdout" {
            if !(1..=2).contains(&args.len()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'capturar_stdout' com aridade inválida: esperado 1 ou 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let command_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(command_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'capturar_stdout': esperado 'verso', encontrado '{}'",
                        command_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            if args.len() == 2 {
                let argv1_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(argv1_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada 'capturar_stdout': esperado 'verso', encontrado '{}'",
                            argv1_ty.name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(Type::Verso(expr_span));
        }

        // Fase 170 — capturar_stderr(comando[, argv1]) -> verso
        if name == "capturar_stderr" {
            if !(1..=2).contains(&args.len()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'capturar_stderr' com aridade inválida: esperado 1 ou 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let command_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(command_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'capturar_stderr': esperado 'verso', encontrado '{}'",
                        command_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            if args.len() == 2 {
                let argv1_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(argv1_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada 'capturar_stderr': esperado 'verso', encontrado '{}'",
                            argv1_ty.name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(Type::Verso(expr_span));
        }

        if name == "__pinker_internal_mapa_verso_verso_iterador_criar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<verso,verso> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let map_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(map_ty, Type::MapVersoVerso(_)) {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<verso,verso> exige mapa<verso,verso>"
                        .to_string(),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "__pinker_internal_mapa_verso_verso_iterador_proxima_chave" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<verso,verso> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            return Ok(Type::Verso(expr_span));
        }

        if name == "__pinker_internal_mapa_bombom_bombom_iterador_criar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,bombom> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let map_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(map_ty, Type::MapBombomBombom(_)) {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,bombom> exige mapa<bombom,bombom>"
                        .to_string(),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,bombom> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            return Ok(Type::Bombom(expr_span));
        }

        if name == "__pinker_internal_mapa_bombom_verso_iterador_criar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,verso> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let map_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(map_ty, Type::MapBombomVerso(_)) {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,verso> exige mapa<bombom,verso>"
                        .to_string(),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,verso> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            return Ok(Type::Bombom(expr_span));
        }

        let arg_refs: Vec<&Expr> = args.iter().collect();
        self.check_named_function_call(expr_span, callee.span, name, &arg_refs)
    }
    // @pinker-nav:end semantic.chamadas.despacho
}
