//! Verificação de expressões, de fluxo/retornos e de `encaixe` de união da
//! checagem semântica, movida de `src/semantic.rs` pela unidade SEM-3 do
//! inventário da #601 (Task #634).
//!
//! Só o arquivo mudou: as três regiões cartografadas
//! `semantic.unioes.encaixe`, `semantic.fluxo.retornos` e
//! `semantic.expressoes.verificacao`, as onze funções que as compõem e a ordem
//! em que elas decidem continuam exatamente como estavam. `super` mudou de
//! significado ao descer um nível, e o `use` abaixo devolve ao irmão o
//! vocabulário do pai — `SemanticChecker`, os tipos da AST e os helpers
//! privados — sem promover nada: um filho enxerga os itens privados do pai por
//! privacidade de módulo, e este `use` é privado.
//!
//! As três regiões vinham contíguas no pai e continuam contíguas aqui, na
//! mesma ordem: `encaixe` de união, fluxo e retornos, verificação de
//! expressões. A vizinhança que a fase enxerga não mudou de forma nenhuma.
//!
//! O corte não atravessa autoridade nenhuma. Nada aqui consulta
//! `method_dispatch` (C2) — a única consulta da fase, `select_impl_method`,
//! desceu com a SEM-1 e mora em `calls.rs`; `select_representative` continua
//! na família de tratos, no pai. Nada aqui lê o registry declarativo de
//! intrínsecas (C1), reconstrói origem de default body por grafia (C5),
//! reabre a conclusão arquitetural da #600 (C6) ou decide a política de
//! alcance ainda aberta da #579. O estado (`SemanticChecker`), a ordem das
//! duas passagens, os escopos, o sistema de tipos e as demais famílias
//! continuam no pai.
//!
//! Nenhum item do corte era `pub` antes do move e nenhum é agora. Sete dos
//! onze símbolos passaram de privados a `pub(super)` — `check_union_match`,
//! `check_if_as_nested_branch`, `check_return_stmt`, `block_returns`,
//! `check_value_expr`, `function_result_type` e `check_expr` —, que são
//! exatamente os que o pai ou um irmão chamam. Os outros quatro
//! (`if_returns`, `enum_match_returns`, `union_match_returns` e
//! `check_pointer_arithmetic`) só têm chamadores dentro do próprio corte e
//! continuam privados.

use super::*;

impl SemanticChecker {
    // @pinker-nav:start semantic.unioes.encaixe
    // @pinker-nav:domain unioes
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Verificação de `encaixe` de união: resolve o tipo do scrutinee e o tipo de cada braço integralmente (apelidos inclusos), deriva a chave canônica compartilhada de `union_canon`, exige que cada braço pertença à união, rejeita duplicata após a resolução (dois apelidos do mesmo tipo canônico são o mesmo membro), exige cobertura exata dos membros canônicos e abre um escopo por braço com o binding declarado no tipo resolvido do membro. Nenhuma tag é calculada ou armazenada aqui — a tag pertence ao registry internado pelo lowering.
    pub(super) fn check_union_match(
        &mut self,
        union_match: &UnionMatchStmt,
    ) -> Result<(), PinkerError> {
        let scrutinee_ty = self.check_value_expr(
            &union_match.scrutinee,
            "resultado de função sem retorno não pode ser inspecionado por 'encaixe'",
        )?;
        let scrutinee_ty = self.resolve_type_or_error(&scrutinee_ty)?;
        let Type::Union {
            members: canonical_members,
            ..
        } = &scrutinee_ty
        else {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "'encaixe' de união exige scrutinee de união estrutural; encontrado '{}'",
                    scrutinee_ty.name()
                ),
                span: union_match.scrutinee.span,
            });
        };

        // Cada braço é associado ao membro canônico pelo **tipo resolvido**.
        // O spelling original (o nome do apelido escrito) é preservado apenas
        // para diagnóstico.
        let mut covered = HashSet::<String>::new();
        let mut arm_members = Vec::with_capacity(union_match.arms.len());
        for arm in &union_match.arms {
            let resolved_member = self.resolve_type_or_error(&arm.member_type)?;
            let key = union_canon::member_key(&resolved_member);
            let Some(index) = union_canon::canonical_member_index(canonical_members, &key) else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "braço '{}' de 'encaixe' não é membro da união '{}'",
                        arm.member_type.name(),
                        scrutinee_ty.name()
                    ),
                    span: arm.span,
                });
            };
            if !covered.insert(key.canonical_type_key.clone()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "membro '{}' repetido no 'encaixe' de união após resolução de apelidos",
                        canonical_members[index].name()
                    ),
                    span: arm.span,
                });
            }
            arm_members.push(canonical_members[index].clone());
        }

        if covered.len() != canonical_members.len() {
            let faltantes = canonical_members
                .iter()
                .filter(|member| !covered.contains(union_canon::member_key(member).as_str()))
                .map(|member| member.name().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(PinkerError::Semantic {
                msg: format!(
                    "encaixe de união deve ser exaustivo: os braços devem cobrir exatamente todos os membros canônicos; ausente(s): {faltantes}"
                ),
                span: union_match.span,
            });
        }

        for (arm, member_ty) in union_match.arms.iter().zip(arm_members) {
            self.push_scope();
            let checked = self
                .declare_var(&arm.binding, member_ty.with_span(arm.span), false, arm.span)
                .and_then(|()| self.check_block(&arm.body, true));
            self.pop_scope();
            checked?;
        }

        Ok(())
    }
    // @pinker-nav:end semantic.unioes.encaixe

    // @pinker-nav:start semantic.fluxo.retornos
    // @pinker-nav:domain fluxo
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Fluxo e retornos: verificação de ramo `talvez`/`senão` aninhado com escopo próprio, checagem de `mimo` de retorno contra o tipo declarado (presença/ausência de valor, tipo e faixa) e análise superficial de alcançabilidade — um bloco retorna se contém `mimo` direto ou uma seleção exaustiva (`talvez`/`senão` ou `encaixe`) em que todos os braços retornam.
    pub(super) fn check_if_as_nested_branch(
        &mut self,
        if_stmt: &IfStmt,
    ) -> Result<(), PinkerError> {
        self.push_scope();
        let cond_ty = self.check_value_expr(
            &if_stmt.condition,
            "condição não pode usar resultado de função sem retorno",
        )?;
        if !matches!(cond_ty, Type::Logica(_)) {
            self.pop_scope();
            return Err(PinkerError::Semantic {
                msg: "condição de 'talvez' deve ser 'logica'".to_string(),
                span: if_stmt.condition.span,
            });
        }

        self.check_block(&if_stmt.then_branch, false)?;
        if let Some(else_branch) = &if_stmt.else_branch {
            match else_branch {
                ElseBlock::Block(block) => self.check_block(block, false)?,
                ElseBlock::If(inner) => self.check_if_as_nested_branch(inner)?,
            }
        }
        self.pop_scope();
        Ok(())
    }

    pub(super) fn check_return_stmt(
        &mut self,
        return_stmt: &ReturnStmt,
    ) -> Result<(), PinkerError> {
        let current_ret = self.current_func_ret.clone();
        match (current_ret, &return_stmt.expr) {
            (None, None) => Ok(()),
            (None, Some(_)) => Err(PinkerError::Semantic {
                msg: "mimo com valor não é permitido em função sem retorno declarado".to_string(),
                span: return_stmt.span,
            }),
            (Some(_), None) => Err(PinkerError::Semantic {
                msg: "mimo sem valor não é permitido em função com retorno declarado".to_string(),
                span: return_stmt.span,
            }),
            (Some(expected), Some(expr)) => {
                let value_ty = self.check_value_expr(
                    expr,
                    "resultado de função sem retorno não pode ser retornado como valor",
                )?;
                if !Self::check_expected_type_for_expr(&expected, &value_ty, expr) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "retorno incompatível em '{}': esperado '{}', encontrado '{}'",
                            self.current_func_name
                                .as_deref()
                                .unwrap_or("<desconhecida>"),
                            expected.name(),
                            value_ty.name()
                        ),
                        span: expr.span,
                    });
                }
                Self::validate_int_literal_range(&expected, expr)?;
                Ok(())
            }
        }
    }

    // Análise de alcançabilidade de retorno superficial: verifica se o bloco
    // contém um `mimo` direto ou uma seleção exaustiva onde todos os ramos retornam.
    // Não analisa fluxo complexo nem condições de laço — suficiente para a v0.
    pub(super) fn block_returns(&self, block: &Block) -> bool {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Return(_) => return true,
                Stmt::If(if_stmt) if self.if_returns(if_stmt) => return true,
                Stmt::EnumMatch(enum_match) if self.enum_match_returns(enum_match) => return true,
                Stmt::UnionMatch(union_match) if self.union_match_returns(union_match) => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn if_returns(&self, if_stmt: &IfStmt) -> bool {
        let then_returns = self.block_returns(&if_stmt.then_branch);
        let else_returns = match &if_stmt.else_branch {
            Some(ElseBlock::Block(block)) => self.block_returns(block),
            Some(ElseBlock::If(inner)) => self.if_returns(inner),
            None => false,
        };
        then_returns && else_returns
    }

    fn enum_match_returns(&self, enum_match: &EnumMatchStmt) -> bool {
        let arms_return = !enum_match.arms.is_empty()
            && enum_match
                .arms
                .iter()
                .all(|arm| self.block_returns(&arm.body));
        let otherwise_returns = match &enum_match.otherwise {
            Some(otherwise) => self.block_returns(otherwise),
            None => true,
        };
        arms_return && otherwise_returns
    }

    fn union_match_returns(&self, union_match: &UnionMatchStmt) -> bool {
        !union_match.arms.is_empty()
            && union_match
                .arms
                .iter()
                .all(|arm| self.block_returns(&arm.body))
    }
    // @pinker-nav:end semantic.fluxo.retornos

    // @pinker-nav:start semantic.expressoes.verificacao
    // @pinker-nav:domain expressoes
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Verificação de expressões que produz o tipo de cada nó: exigência de valor não-`Nulo` (`check_value_expr`), tipo de resultado de função, e o despacho central (`check_expr`) sobre literais, identificadores, cursores internos de mapa, acesso a campo/variante de leque, indexação, `virar` (cast), `peso`/`alinhamento`, operações binárias (incluindo aritmética de ponteiro) e unárias (negação, `nao`, bitwise, dereferência).
    pub(super) fn check_value_expr(
        &mut self,
        expr: &Expr,
        void_message: &str,
    ) -> Result<Type, PinkerError> {
        let ty = self.check_expr(expr)?;
        if ty.is_nulo() {
            return Err(PinkerError::Semantic {
                msg: void_message.to_string(),
                span: expr.span,
            });
        }
        Ok(ty)
    }

    // `Nulo` existe só internamente para a semântica da v0: função sem `-> tipo` retorna `Nulo`.
    // Esse tipo nunca pode aparecer em declaração de usuário.
    pub(super) fn function_result_type(&self, function: &FunctionDecl, span: Span) -> Type {
        let base = function
            .ret_type
            .as_ref()
            .and_then(|ty| self.resolve_type_or_error(ty).ok())
            .unwrap_or(Type::Nulo(span));
        base.with_span(span)
    }

    pub(super) fn check_expr(&mut self, expr: &Expr) -> Result<Type, PinkerError> {
        match &expr.kind {
            ExprKind::IntLit(_) => Ok(Type::Bombom(expr.span)),
            ExprKind::BoolLit(_) => Ok(Type::Logica(expr.span)),
            ExprKind::StringLit(_) => Ok(Type::Verso(expr.span)),
            // #532: intrínseca resolvida fora de posição de chamada. A
            // superfície modular não expõe a intrínseca como VALOR, e a recusa
            // é a mesma que a grafia canônica sempre produziu — sem consultar
            // `resolve_var`, que depois desta Issue poderia encontrar uma
            // função do usuário homônima e deixá-la capturar a referência.
            ExprKind::Intrinsic(identity) => Err(PinkerError::Semantic {
                msg: format!(
                    "identificador '{}' não declarado",
                    identity.canonical_public_spelling()
                ),
                span: expr.span,
            }),
            ExprKind::Ident(name) => {
                // Fase 243: nome sintético de literal `carinho` (Fase 225) —
                // resolve como criação de closure (materializa captura por
                // valor e checa o corpo com o ambiente correto), não como
                // resolução genérica de variável/função da Fase 242.
                if name.starts_with("__anon_carinho_") {
                    return self.resolve_closure_value(name, expr.span);
                }
                self.resolve_var(name)
                    .map(|meta| meta.ty)
                    .ok_or_else(|| PinkerError::Semantic {
                        msg: format!("identificador '{}' não declarado", name),
                        span: expr.span,
                    })
            }
            ExprKind::InternalMapIterCreate(map) => {
                let map_ty =
                    self.check_value_expr(map, "cursor interno de mapa requer mapa como valor")?;
                if !matches!(map_ty, Type::MapVersoBombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "cursor interno de mapa exige 'mapa<verso,bombom>'".to_string(),
                        span: map.span,
                    });
                }
                Ok(Type::Bombom(expr.span))
            }
            ExprKind::InternalMapIterNextKey(iterator) => {
                let iterator_ty = self.check_value_expr(
                    iterator,
                    "avanço interno de iteração de mapa requer cursor como valor",
                )?;
                if !matches!(iterator_ty, Type::Bombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "cursor interno de mapa exige handle 'bombom'".to_string(),
                        span: iterator.span,
                    });
                }
                Ok(Type::Verso(expr.span))
            }
            ExprKind::Call(callee, args) => self.check_call_expr(expr.span, callee, args),
            ExprKind::AddressOf(operand) => {
                let ExprKind::Ident(name) = &operand.kind else {
                    return Err(PinkerError::Semantic {
                        msg: "obtenção de endereço cru exige nome de função top-level".to_string(),
                        span: operand.span,
                    });
                };
                if self.resolve_local_var_type(name).is_some() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "obtenção de endereço cru de '{}' rejeita variável, callable ou closure; use uma função top-level",
                            name
                        ),
                        span: operand.span,
                    });
                }
                if name.starts_with("__anon_carinho_")
                    || name.starts_with("__fnref_env_")
                    || method_identity::parse_provisional_function_name(name).is_some()
                {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "obtenção de endereço cru de '{}' rejeita closure, wrapper de callable ou método",
                            name
                        ),
                        span: operand.span,
                    });
                }
                let Some(function) = self.funcs.get(name).cloned() else {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "símbolo de função '{}' não resolvido para obtenção de endereço cru",
                            name
                        ),
                        span: operand.span,
                    });
                };
                if !function.type_params.is_empty() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "função genérica '{}' exige especialização concreta antes da obtenção de endereço cru",
                            name
                        ),
                        span: operand.span,
                    });
                }
                let signature = Type::Function {
                    params: function
                        .params
                        .iter()
                        .map(|param| param.ty.clone())
                        .collect(),
                    ret: Box::new(
                        function
                            .ret_type
                            .clone()
                            .unwrap_or_else(|| Type::Nulo(function.span)),
                    ),
                    span: function.span,
                };
                self.resolve_type_or_error(&Type::Pointer {
                    base: Box::new(signature),
                    is_volatile: false,
                    span: expr.span,
                })
            }
            ExprKind::FieldAccess { base, field } => {
                // `Leque.Variante` — o nome do leque em posição de base tem
                // precedência sobre variáveis homônimas nesta fase.
                if let ExprKind::Ident(base_name) = &base.kind {
                    if let Some(enum_name) = self.resolve_enum_base_name(base_name) {
                        let enum_decl = self.enums.get(&enum_name).expect("leque resolvido existe");
                        let Some(variant) = enum_decl
                            .variants
                            .iter()
                            .find(|variant| variant.name == *field)
                        else {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "variante '{}' não existe no leque '{}'",
                                    field, base_name
                                ),
                                span: expr.span,
                            });
                        };
                        if !variant.payloads.is_empty() {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "variante '{}' carrega valor; construa com '{}.{}(valor)'",
                                    field, base_name, field
                                ),
                                span: expr.span,
                            });
                        }
                        return Ok(Type::Enum {
                            name: enum_name,
                            span: expr.span,
                        });
                    }
                }
                // Parte G: `familia.membro` sem o `trazer`. Só chega aqui o
                // que o parser NÃO canonicalizou, e só vira erro de família
                // depois de provado que nada mais reivindica o nome.
                if let ExprKind::Ident(base_name) = &base.kind {
                    if let Some(erro) =
                        self.dica_de_familia_nao_importada(base_name, field, expr.span)
                    {
                        return Err(erro);
                    }
                }
                let base_ty = self.check_value_expr(
                    base,
                    "resultado de função sem retorno não pode ser base de acesso a campo",
                )?;
                self.resolve_struct_field_type(&base_ty, field, expr.span)
            }
            ExprKind::Index { base, index } => {
                let base_ty = self.check_value_expr(
                    base,
                    "resultado de função sem retorno não pode ser base de indexação",
                )?;
                let index_ty = self.check_value_expr(
                    index,
                    "resultado de função sem retorno não pode ser índice",
                )?;
                if !matches!(index_ty, Type::Bombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "índice nesta fase deve ser 'bombom'".to_string(),
                        span: index.span,
                    });
                }
                match base_ty {
                    Type::FixedArray { element, .. } => Ok(element.as_ref().with_span(expr.span)),
                    _ => Err(PinkerError::Semantic {
                        msg: "indexação exige base de array fixo nesta fase".to_string(),
                        span: expr.span,
                    }),
                }
            }
            ExprKind::Cast {
                expr: source_expr,
                target,
            } => {
                let source_ty = self.check_value_expr(
                    source_expr,
                    "resultado de função sem retorno não pode ser convertido com 'virar'",
                )?;
                let target_ty = self.resolve_type_or_error(target)?.with_span(expr.span);
                if let Type::Union { members, .. } = &target_ty {
                    let matching = members
                        .iter()
                        .filter(|member| Self::check_type_match(member, &source_ty))
                        .count();
                    return match matching {
                        1 => Ok(target_ty),
                        0 => Err(PinkerError::Semantic {
                            msg: format!(
                                "tipo '{}' não pertence à união estrutural alvo",
                                Self::type_key(&source_ty)
                            ),
                            span: source_expr.span,
                        }),
                        _ => Err(PinkerError::Semantic {
                            msg: "injeção em união estrutural é ambígua; aplique 'virar' explicitamente para o membro desejado antes da união"
                                .to_string(),
                            span: source_expr.span,
                        }),
                    };
                }
                if matches!(source_ty, Type::Union { .. }) {
                    return Err(PinkerError::Semantic {
                        msg: "downcast de união fora de 'encaixe' não é permitido".to_string(),
                        span: source_expr.span,
                    });
                }
                if let Some(trait_name) = Self::trait_object_name(&target_ty).map(str::to_string) {
                    let supported_concrete = matches!(
                        &source_ty,
                        Type::Bombom(_)
                            | Type::U8(_)
                            | Type::U16(_)
                            | Type::U32(_)
                            | Type::U64(_)
                            | Type::I8(_)
                            | Type::I16(_)
                            | Type::I32(_)
                            | Type::I64(_)
                            | Type::Logica(_)
                            | Type::Verso(_)
                            | Type::Struct { .. }
                    );
                    if !supported_concrete {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "objeto de trato nesta fase aceita tipo concreto escalar ou ninho; encontrado '{}'",
                                Self::type_key(&source_ty)
                            ),
                            span: source_expr.span,
                        });
                    }

                    let source_direct = Self::type_key(&source_ty);
                    let source_identity = self.resolved_type_identity(&source_ty)?;

                    let has_impl = self.impl_methods.iter().any(|meta| {
                        meta.identity.trait_name == trait_name
                            && meta.identity.target == source_identity
                    });

                    if !has_impl {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "tipo '{}' não implementa o trato '{}' e não pode formar '{}'",
                                source_direct,
                                trait_name,
                                Self::type_key(&target_ty)
                            ),
                            span: source_expr.span,
                        });
                    }

                    // #649 — é aqui que a relação CONCRETA entra na
                    // representação dinâmica: a vtable do objeto é construída a
                    // partir dela, e a chamada dinâmica posterior só consome o
                    // que este ponto autorizou. O alcance é perguntado uma vez,
                    // na formação, à mesma autoridade de `module_resolve`;
                    // `lower_trait_call` não refaz busca modular alguma.
                    if !self.relacao_alcanca(&trait_name, &source_identity, expr.span) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "impl de '{}' para tipo '{}' não é alcançável desta unidade e não pode formar '{}'; \
                                 importe a unidade que declara essa implementação",
                                trait_name,
                                source_direct,
                                Self::type_key(&target_ty)
                            ),
                            span: source_expr.span,
                        });
                    }

                    return Ok(target_ty);
                }

                if let Type::Enum { name, .. } = &source_ty {
                    if self.enum_has_payload(name) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "'virar' não é suportado para leque com carga ('{}'); use 'encaixe'",
                                name
                            ),
                            span: expr.span,
                        });
                    }
                }
                if !Self::is_cast_allowed(&source_ty, &target_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "cast explícito inválido nesta fase: '{}' virar '{}'",
                            source_ty.name(),
                            target_ty.name()
                        ),
                        span: expr.span,
                    });
                }
                Ok(target_ty)
            }
            ExprKind::SizeOfType { target } => {
                let resolved = self.resolve_type_or_error(target)?.with_span(expr.span);
                layout::layout_of_type(&resolved, &self.type_aliases, &self.structs).map_err(
                    |msg| PinkerError::Semantic {
                        msg: format!("consulta de peso inválida: {}", msg),
                        span: expr.span,
                    },
                )?;
                Ok(Type::Bombom(expr.span))
            }
            ExprKind::AlignOfType { target } => {
                let resolved = self.resolve_type_or_error(target)?.with_span(expr.span);
                layout::layout_of_type(&resolved, &self.type_aliases, &self.structs).map_err(
                    |msg| PinkerError::Semantic {
                        msg: format!("consulta de alinhamento inválida: {}", msg),
                        span: expr.span,
                    },
                )?;
                Ok(Type::Bombom(expr.span))
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let lhs_ty = self.check_value_expr(
                    lhs,
                    "resultado de função sem retorno não pode ser usado em operação binária",
                )?;
                let rhs_ty = self.check_value_expr(
                    rhs,
                    "resultado de função sem retorno não pode ser usado em operação binária",
                )?;

                if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
                    if let Some(pointer_result) =
                        self.check_pointer_arithmetic(expr.span, *op, &lhs_ty, &rhs_ty, rhs)
                    {
                        return pointer_result;
                    }
                }

                let raw_pointer_null_comparison = if matches!(
                    op,
                    BinaryOp::Eq
                        | BinaryOp::Neq
                        | BinaryOp::Lt
                        | BinaryOp::Lte
                        | BinaryOp::Gt
                        | BinaryOp::Gte
                ) {
                    let lhs_resolved = self.resolve_type_or_error(&lhs_ty)?;
                    let rhs_resolved = self.resolve_type_or_error(&rhs_ty)?;
                    let raw_function_pointer = |ty: &Type| {
                        matches!(
                            ty,
                            Type::Pointer { base, .. }
                                if matches!(base.as_ref(), Type::Function { .. })
                        )
                    };
                    if (raw_function_pointer(&lhs_resolved) || raw_function_pointer(&rhs_resolved))
                        && !matches!(op, BinaryOp::Eq | BinaryOp::Neq)
                    {
                        return Err(PinkerError::Semantic {
                            msg: "ponteiro cru de função aceita apenas igualdade '==' e desigualdade '!='; ordem não possui contrato"
                                .to_string(),
                            span: expr.span,
                        });
                    }
                    if Self::trait_object_name(&lhs_resolved).is_some()
                        || Self::trait_object_name(&rhs_resolved).is_some()
                    {
                        return Err(PinkerError::Semantic {
                            msg: "comparação entre objetos de trato não é suportada: igualdade, ordem e identidade observável ainda não possuem contrato"
                                .to_string(),
                            span: expr.span,
                        });
                    }
                    matches!(op, BinaryOp::Eq | BinaryOp::Neq)
                        && ((raw_function_pointer(&lhs_resolved)
                            && Self::expr_is_zero_literal(rhs))
                            || (raw_function_pointer(&rhs_resolved)
                                && Self::expr_is_zero_literal(lhs)))
                } else {
                    false
                };

                let binary_types_compatible = Self::check_type_match(&lhs_ty, &rhs_ty)
                    || (Self::expr_is_int_literal(lhs) && Self::is_integer_type(&rhs_ty))
                    || (Self::expr_is_int_literal(rhs) && Self::is_integer_type(&lhs_ty))
                    || raw_pointer_null_comparison;
                if !binary_types_compatible {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipos incompatíveis em operação binária: '{}' e '{}'",
                            lhs_ty.name(),
                            rhs_ty.name()
                        ),
                        span: expr.span,
                    });
                }

                match op {
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                        if matches!(lhs_ty, Type::Logica(_)) {
                            Ok(Type::Logica(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "operação lógica requer operandos 'logica'".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::Shl
                    | BinaryOp::Shr => {
                        if Self::is_integer_type(&lhs_ty) {
                            if Self::expr_is_int_literal(lhs)
                                && !Self::expr_is_int_literal(rhs)
                                && Self::is_integer_type(&rhs_ty)
                            {
                                Ok(rhs_ty.with_span(expr.span))
                            } else {
                                Ok(lhs_ty.with_span(expr.span))
                            }
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "operação aritmética/bitwise requer operandos inteiros compatíveis"
                                    .to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    BinaryOp::Eq | BinaryOp::Neq => {
                        if matches!(&lhs_ty, Type::Union { .. }) {
                            return Err(PinkerError::Semantic {
                                msg: "igualdade e desigualdade de união estrutural não são suportadas nesta fase; use 'encaixe'"
                                    .to_string(),
                                span: expr.span,
                            });
                        }
                        if let Type::Enum { name, .. } = &lhs_ty {
                            if self.enum_has_payload(name) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "igualdade direta não é suportada para leque com carga ('{}'); use 'encaixe'",
                                        name
                                    ),
                                    span: expr.span,
                                });
                            }
                        }
                        Ok(Type::Logica(expr.span))
                    }
                    BinaryOp::Lt | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Gte => {
                        if matches!(&lhs_ty, Type::Union { .. }) {
                            return Err(PinkerError::Semantic {
                                msg: "comparação de ordem não é suportada para união estrutural; use 'encaixe'"
                                    .to_string(),
                                span: expr.span,
                            });
                        }
                        if matches!(lhs_ty, Type::Enum { .. }) {
                            return Err(PinkerError::Semantic {
                                msg: "comparação de ordem não é suportada entre valores de leque; use '==' ou '!='"
                                    .to_string(),
                                span: expr.span,
                            });
                        }
                        Ok(Type::Logica(expr.span))
                    }
                }
            }
            ExprKind::Unary(op, operand) => {
                let inner_ty = self.check_value_expr(
                    operand,
                    "resultado de função sem retorno não pode ser usado em operação unária",
                )?;
                match op {
                    UnaryOp::Neg => {
                        if Self::is_integer_type(&inner_ty) {
                            Ok(inner_ty.with_span(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação aritmética requer operando inteiro".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    UnaryOp::Not => {
                        if matches!(inner_ty, Type::Logica(_)) {
                            Ok(Type::Logica(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação lógica requer operando 'logica'".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    UnaryOp::BitNot => {
                        if Self::is_integer_type(&inner_ty) {
                            Ok(inner_ty.with_span(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação bitwise requer operando inteiro".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    UnaryOp::Deref => match inner_ty {
                        Type::Pointer { base, .. } => match base.as_ref() {
                            Type::Bombom(_)
                            | Type::U8(_)
                            | Type::U16(_)
                            | Type::U32(_)
                            | Type::U64(_)
                            | Type::I8(_)
                            | Type::I16(_)
                            | Type::I32(_)
                            | Type::I64(_)
                            | Type::Logica(_) => Ok(base.as_ref().clone().with_span(expr.span)),
                            Type::FixedArray { element, size, .. }
                                if matches!(element.as_ref(), Type::Bombom(_)) =>
                            {
                                Ok(Type::FixedArray {
                                    element: Box::new(Type::Bombom(expr.span)),
                                    size: *size,
                                    span: expr.span,
                                })
                            }
                            Type::Struct { name, .. } => Ok(Type::Struct {
                                name: name.clone(),
                                span: expr.span,
                            }),
                            _ => Err(PinkerError::Semantic {
                                msg: "dereferência aceita ponteiro para escalar público, array suportado ou ninho".to_string(),
                                span: expr.span,
                            }),
                        },
                        _ => Err(PinkerError::Semantic {
                            msg: "dereferência requer operando do tipo 'seta<T>'".to_string(),
                            span: expr.span,
                        }),
                    },
                }
            }
        }
    }

    fn check_pointer_arithmetic(
        &self,
        expr_span: Span,
        op: BinaryOp,
        lhs_ty: &Type,
        rhs_ty: &Type,
        rhs_expr: &Expr,
    ) -> Option<Result<Type, PinkerError>> {
        let is_bombom = |ty: &Type| matches!(ty, Type::Bombom(_));

        if let Type::Pointer { base, .. } = lhs_ty {
            if is_bombom(rhs_ty) {
                if op == BinaryOp::Sub {
                    return Some(if matches!(base.as_ref(), Type::Bombom(_)) {
                        Ok(lhs_ty.with_span(expr_span))
                    } else {
                        Err(PinkerError::Semantic {
                            msg: "subtração de ponteiro preserva somente o contrato legado 'seta<bombom> - bombom'; D5 não amplia esta operação".to_string(),
                            span: expr_span,
                        })
                    });
                }

                if matches!(rhs_expr.kind, ExprKind::Unary(UnaryOp::Neg, _)) {
                    return Some(Err(PinkerError::Semantic {
                        msg: "deslocamento de seta<T> deve ser 'bombom' não negativo".to_string(),
                        span: rhs_expr.span,
                    }));
                }

                let supported = matches!(
                    base.as_ref(),
                    Type::Bombom(_)
                        | Type::U8(_)
                        | Type::U16(_)
                        | Type::U32(_)
                        | Type::U64(_)
                        | Type::I8(_)
                        | Type::I16(_)
                        | Type::I32(_)
                        | Type::I64(_)
                        | Type::Logica(_)
                        | Type::Struct { .. }
                ) || matches!(
                    base.as_ref(),
                    Type::FixedArray { element, .. }
                        if matches!(element.as_ref(), Type::Bombom(_))
                );
                if !supported {
                    return Some(Err(PinkerError::Semantic {
                        msg: format!(
                            "aritmética de seta<T> exige elemento com layout e acesso coerentes; '{}' não participa de D5",
                            base.name()
                        ),
                        span: expr_span,
                    }));
                }

                let element_layout =
                    match layout::layout_of_type(base, &self.type_aliases, &self.structs) {
                        Ok(layout) => layout,
                        Err(msg) => {
                            return Some(Err(PinkerError::Semantic {
                                msg: format!(
                                    "aritmética de seta<T> exige layout canônico conhecido: {}",
                                    msg
                                ),
                                span: expr_span,
                            }))
                        }
                    };
                if element_layout.size == 0
                    || element_layout.align == 0
                    || element_layout.size % element_layout.align != 0
                {
                    return Some(Err(PinkerError::Semantic {
                        msg: "layout de elemento inválido para aritmética de seta<T>".to_string(),
                        span: expr_span,
                    }));
                }
                if let ExprKind::IntLit(offset) = rhs_expr.kind {
                    if offset.checked_mul(element_layout.size).is_none() {
                        return Some(Err(PinkerError::Semantic {
                            msg: "E-POINTER-OFFSET-OVERFLOW: overflow ao escalar deslocamento de seta<T>"
                                .to_string(),
                            span: rhs_expr.span,
                        }));
                    }
                }
                return Some(Ok(lhs_ty.with_span(expr_span)));
            }
        }
        if is_bombom(lhs_ty) && matches!(rhs_ty, Type::Pointer { .. }) {
            let msg = match op {
                BinaryOp::Add => "aritmética de ponteiro suporta apenas 'seta<T> + bombom'",
                BinaryOp::Sub => "subtração de ponteiro nesta fase suporta apenas 'ptr - bombom'",
                _ => unreachable!("check_pointer_arithmetic só recebe add/sub"),
            };
            return Some(Err(PinkerError::Semantic {
                msg: msg.to_string(),
                span: expr_span,
            }));
        }
        if matches!(lhs_ty, Type::Pointer { .. }) || matches!(rhs_ty, Type::Pointer { .. }) {
            let msg = match op {
                BinaryOp::Add => "aritmética de ponteiro exige 'seta<T> + bombom'",
                BinaryOp::Sub => "aritmética de ponteiro nesta fase exige 'seta<bombom> - bombom'",
                _ => unreachable!("check_pointer_arithmetic só recebe add/sub"),
            };
            return Some(Err(PinkerError::Semantic {
                msg: msg.to_string(),
                span: expr_span,
            }));
        }
        None
    }
    // @pinker-nav:end semantic.expressoes.verificacao
}
