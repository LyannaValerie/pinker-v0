//! Relações, métodos e contratos de tratos da checagem semântica, movidos de
//! `src/semantic.rs` pela unidade SEM-4 do inventário da #601 (Task #636).
//!
//! Só o arquivo mudou: a região cartografada `semantic.tratos.contratos`, as
//! sete funções que a compõem e a ordem em que elas decidem continuam
//! exatamente como estavam. `super` mudou de significado ao descer um nível, e
//! o `use` abaixo devolve ao irmão o vocabulário do pai — `SemanticChecker`, os
//! tipos da AST e os helpers privados — sem promover nada: um filho enxerga os
//! itens privados do pai por privacidade de módulo, e este `use` é privado.
//!
//! A ordem das quatro decisões da fase não mudou de lugar: o pai continua
//! chamando `validate_impl_relations`, `register_impl_methods`,
//! `validate_impl_contracts` e `validate_trait_contracts` nessa ordem, na
//! região `semantic.programa.duas-passagens`. A vizinhança que a fase enxerga
//! é a mesma.
//!
//! O corte atravessa C2 e não a muda. `src/method_dispatch.rs` (#590/#591)
//! continua sendo a autoridade única de seleção: a consulta a
//! `select_representative` desceu junto com a região que sempre a fez, e
//! continua sendo uma só na fase semântica — a outra, `select_impl_method`,
//! desceu com a SEM-1 e mora em `calls.rs`. Aqui fica o que sempre ficou: a
//! construção dos candidatos e a mensagem, que é da fase. Nada aqui reconstrói
//! origem de default body por grafia (C5, #592/#593) — `__impl_` e
//! `__trait_default_check_` aparecem só como transporte que a identidade
//! canônica ignora, exatamente como antes —, nada lê o registry declarativo de
//! intrínsecas (C1), nada reabre a conclusão arquitetural da #600 (C6) e nada
//! decide a política de alcance ainda aberta da #579. O estado
//! (`SemanticChecker`), a ordem das duas passagens, os escopos, o sistema de
//! tipos e as demais famílias continuam no pai.
//!
//! Nenhum item do corte era `pub` antes do move e nenhum é agora. Cinco das
//! sete funções passaram de privadas a `pub(super)` —
//! `validate_impl_relations`, `register_impl_methods`,
//! `validate_impl_contracts`, `validate_object_trait_shape` e
//! `validate_trait_contracts` —, que são exatamente as que o pai chama. As
//! outras duas (`validate_impl_trait_method_function` e
//! `validate_trait_method_function`) só têm chamadores dentro do próprio corte
//! e continuam privadas.

use super::*;

impl SemanticChecker {
    // @pinker-nav:start semantic.tratos.contratos
    // @pinker-nav:domain tratos
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Autoridade semântica de relações, métodos e contratos de tratos. `validate_impl_relations` vem primeiro e é a única autoridade de cardinalidade da relação nominal: cada `ImplDecl` de `program.impls` vira a identidade `(trato canônico, alvo canônico)` — o mesmo `union_canon` que a identidade de método usa — e a segunda declaração da mesma identidade é recusada, sem olhar quantos métodos explícitos cada bloco materializou; bloco vazio continua sendo declaração da relação. Depois, `register_impl_methods` resolve integralmente o tipo-alvo declarado transportado em `ImplFunctionFacts`, deriva sua chave por `union_canon`, registra `MethodIdentity(trato, tipo resolvido, método)` e compara separadamente o receiver resolvido; `method_index` é somente a visão derivada para chamadas não qualificadas, e a recusa de método repetido continua endereçando repetição dentro do mesmo bloco. Qual das funções já materializadas representa a identidade — override explícito vence default, ordem total do símbolo desempata — é dito por `method_dispatch`, a mesma autoridade que o lowering consulta; aqui fica só a mensagem, que é da fase. Por último, `validate_impl_contracts` agrupa os métodos já materializados pela identidade resolvida e cobra cobertura do contrato do trato: ausência de método requerido é erro de cobertura, nunca duplicata.
    /// Cardinalidade da relação nominal de `impl`, antes de qualquer
    /// materialização de método.
    ///
    /// A relação existe porque a declaração existe: `impl T para X {}` é a
    /// mesma relação que `impl T para X { ... }`, e duas declarações da mesma
    /// identidade canônica `(trato, alvo)` são uma duplicata mesmo quando uma
    /// delas não escreve método algum. Derivar isto da contagem de métodos
    /// materializados era o que fazia um bloco sem método explícito
    /// desaparecer da coerência.
    ///
    /// A identidade vem das autoridades canônicas já existentes: o
    /// `trait_name` que a resolução modular canonizou e a chave de
    /// `union_canon` do alvo resolvido. Nome sintético (`__impl_*`,
    /// `__trait_default_check_*`) é transporte e não participa desta decisão.
    pub(super) fn validate_impl_relations(&mut self, program: &Program) -> Result<(), PinkerError> {
        // (trato canônico, alvo canônico) -> (grafia resolvida, grafia
        // declarada, span da primeira declaração)
        let mut declared: BTreeMap<(String, String), (String, String, Span)> = BTreeMap::new();
        let mut origens: HashMap<(String, String), SourceId> = HashMap::new();
        for impl_decl in &program.impls {
            let declared_spelling = Self::type_key(&impl_decl.target_ty);
            let resolved = self.resolve_type_or_error(&impl_decl.target_ty)?;
            let canonical = union_canon::canonical_type_key(&resolved);
            if union_canon::is_poisoned_key(&canonical) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "identidade resolvida do alvo '{}' do impl '{}' foi perdida",
                        declared_spelling, impl_decl.trait_name
                    ),
                    span: impl_decl.target_ty.span(),
                });
            }
            let resolved_display = Self::type_key(&resolved);
            match declared.entry((impl_decl.trait_name.clone(), canonical)) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    // #577: a mesma identidade que decide cardinalidade decide
                    // origem. Uma segunda declaração nunca chega aqui, então a
                    // relação tem exatamente uma unidade declarante.
                    origens.insert(
                        (impl_decl.trait_name.clone(), slot.key().1.clone()),
                        impl_decl.span.source,
                    );
                    slot.insert((resolved_display, declared_spelling, impl_decl.span));
                }
                std::collections::btree_map::Entry::Occupied(slot) => {
                    let (previous_resolved, previous_spelling, previous_span) = slot.get();
                    if previous_spelling == &declared_spelling {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "impl do trato '{}' para tipo '{}' já declarado; outra declaração em {}",
                                impl_decl.trait_name, declared_spelling, previous_span
                            ),
                            span: impl_decl.span,
                        });
                    }
                    let equivalence = format!(
                        "'{}' e '{}' resolvem para '{}'",
                        declared_spelling, previous_spelling, previous_resolved
                    );
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "impl do trato '{}' para tipo '{}' conflita com impl para '{}'; {} (outra declaração em {})",
                            impl_decl.trait_name,
                            declared_spelling,
                            previous_spelling,
                            equivalence,
                            previous_span
                        ),
                        span: impl_decl.span,
                    });
                }
            }
        }
        self.fontes_das_relacoes = origens;
        Ok(())
    }

    pub(super) fn register_impl_methods(&mut self, program: &Program) -> Result<(), PinkerError> {
        let mut candidates: BTreeMap<MethodIdentity<String>, Vec<ImplMethodMeta>> = BTreeMap::new();
        for item in &program.items {
            let Item::Function(function) = item else {
                continue;
            };
            let Some((trait_name, _target_transport, method_name)) =
                method_identity::parse_provisional_function_name(&function.name)
            else {
                continue;
            };
            let impl_facts = function
                .impl_facts
                .as_ref()
                .ok_or_else(|| PinkerError::Semantic {
                    msg: format!(
                        "método provisório '{}.{}' perdeu o alvo declarado do impl",
                        trait_name, method_name
                    ),
                    span: function.span,
                })?;
            let target_spelling = Self::type_key(&impl_facts.target_ty);
            let resolved_target = self.resolve_type_or_error(&impl_facts.target_ty)?;
            let canonical_target = union_canon::canonical_type_key(&resolved_target);
            if union_canon::is_poisoned_key(&canonical_target) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "identidade resolvida do alvo '{}' do impl '{}' foi perdida",
                        target_spelling, trait_name
                    ),
                    span: impl_facts.target_ty.span(),
                });
            }
            let resolved_target_display = Self::type_key(&resolved_target);
            let identity = MethodIdentity::new(trait_name, canonical_target, method_name);
            candidates
                .entry(identity.clone())
                .or_default()
                .push(ImplMethodMeta {
                    identity,
                    target_spelling,
                    resolved_target_display,
                    function_name: function.name.clone(),
                    is_generated_default: function.e_default_selecionado(),
                    span: function.span,
                });
        }

        for (identity, mut candidates) in candidates {
            let selecao = method_dispatch::select_representative(
                &mut candidates,
                |candidate| &candidate.function_name,
                |candidate| candidate.is_generated_default,
            );
            let escolhido = match selecao {
                RepresentativeSelection::Selected(index) => index,
                RepresentativeSelection::ExplicitConflict {
                    previous,
                    conflicting,
                } => {
                    let previous = &candidates[previous];
                    let conflicting = &candidates[conflicting];
                    if previous.target_spelling == conflicting.target_spelling {
                        return Err(PinkerError::Semantic {
                        msg: format!(
                            "método '{}' do trato '{}' para tipo '{}' já implementado; outra implementação já declarada em {}",
                            identity.method_name,
                            identity.trait_name,
                            conflicting.target_spelling,
                            previous.span
                        ),
                        span: conflicting.span,
                    });
                    }
                    let equivalence = format!(
                        "'{}' e '{}' resolvem para '{}'",
                        conflicting.target_spelling,
                        previous.target_spelling,
                        conflicting.resolved_target_display
                    );
                    return Err(PinkerError::Semantic {
                    msg: format!(
                        "método '{}' do trato '{}' para tipo '{}' conflita com implementação para '{}'; {} (outra declaração em {})",
                        identity.method_name,
                        identity.trait_name,
                        conflicting.target_spelling,
                        previous.target_spelling,
                        equivalence,
                        previous.span
                    ),
                    span: conflicting.span,
                });
                }
            };

            // A escolha do representante — override explícito vence default
            // materializado, ordem total do símbolo desempata — é de
            // `method_dispatch`; aqui só sobra a mensagem, que é da fase.
            let selected = candidates[escolhido].clone();
            self.method_index
                .entry((identity.target.clone(), identity.method_name.clone()))
                .or_default()
                .push(selected.function_name.clone());
            self.impl_methods.push(selected);
        }
        Ok(())
    }

    pub(super) fn validate_impl_contracts(&self, program: &Program) -> Result<(), PinkerError> {
        let mut groups: BTreeMap<(String, String), (String, Vec<&ImplMethodMeta>)> =
            BTreeMap::new();
        for impl_decl in &program.impls {
            let resolved = self.resolve_type_or_error(&impl_decl.target_ty)?;
            let canonical = union_canon::canonical_type_key(&resolved);
            groups
                .entry((impl_decl.trait_name.clone(), canonical))
                .or_insert_with(|| (Self::type_key(&resolved), Vec::new()));
        }
        for meta in &self.impl_methods {
            groups
                .entry((
                    meta.identity.trait_name.clone(),
                    meta.identity.target.clone(),
                ))
                .or_insert_with(|| (meta.resolved_target_display.clone(), Vec::new()))
                .1
                .push(meta);
        }

        for ((trait_name, _canonical_target), (target_type, methods)) in groups {
            let Some(trait_decl) = self.traits.get(&trait_name) else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "impl '{}' para '{}' referencia trato não declarado",
                        trait_name, target_type
                    ),
                    span: Span::new(Position::new(0, 0), Position::new(0, 0)),
                });
            };
            // Preserve the trait's contextual-`si` diagnostics before
            // contextualizing and comparing concrete impl signatures.
            self.validate_object_trait_shape(trait_decl)?;
            let mut seen = HashSet::new();

            for meta in &methods {
                let Some(method) = trait_decl
                    .methods
                    .iter()
                    .find(|method| method.name == meta.identity.method_name)
                else {
                    let span = self
                        .funcs
                        .get(&meta.function_name)
                        .map(|function| function.span)
                        .unwrap_or(trait_decl.span);
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "impl '{}' para '{}' declara método '{}' que não existe no trato",
                            trait_name, target_type, meta.identity.method_name
                        ),
                        span,
                    });
                };
                if !seen.insert(meta.identity.method_name.as_str()) {
                    let span = self
                        .funcs
                        .get(&meta.function_name)
                        .map(|function| function.span)
                        .unwrap_or(trait_decl.span);
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "impl '{}' para '{}' declara método '{}' mais de uma vez",
                            trait_name, target_type, meta.identity.method_name
                        ),
                        span,
                    });
                }

                let function = self
                    .funcs
                    .get(&meta.function_name)
                    .expect("impl method metadata always references a collected function");
                self.validate_impl_trait_method_function(trait_decl, method, meta, function)?;
            }

            for method in &trait_decl.methods {
                if !seen.contains(method.name.as_str()) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "impl '{}' para '{}' não implementa método '{}'",
                            trait_name, target_type, method.name
                        ),
                        span: method.span,
                    });
                }
            }
        }

        Ok(())
    }

    pub(super) fn validate_object_trait_shape(
        &self,
        trait_decl: &TraitDecl,
    ) -> Result<bool, PinkerError> {
        let uses_contextual_self = trait_decl.methods.iter().any(|method| {
            method
                .params
                .iter()
                .any(|param| Self::type_contains_contextual_self(&param.ty))
                || method
                    .ret_type
                    .as_ref()
                    .map(Self::type_contains_contextual_self)
                    .unwrap_or(false)
        });

        if !uses_contextual_self {
            return Ok(false);
        }

        for method in &trait_decl.methods {
            let Some(receiver) = method.params.first() else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "trato '{}' usa receiver contextual 'si'; método '{}' deve declarar 'si' como primeiro parâmetro",
                        trait_decl.name, method.name
                    ),
                    span: method.span,
                });
            };

            if !Self::is_contextual_self_type(&receiver.ty) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "trato '{}' usa receiver contextual 'si'; método '{}' deve declarar 'si' como primeiro parâmetro",
                        trait_decl.name, method.name
                    ),
                    span: receiver.span,
                });
            }

            for param in method.params.iter().skip(1) {
                if Self::type_contains_contextual_self(&param.ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "método '{}' do trato '{}' só pode usar 'si' como primeiro parâmetro receiver",
                            method.name, trait_decl.name
                        ),
                        span: param.span,
                    });
                }

                let struct_names = self.structs.keys().cloned().collect::<HashSet<_>>();
                let ir_type = TypeIR::from_ast_with_context(
                    &param.ty,
                    &self.type_aliases,
                    &struct_names,
                )
                .map_err(|error| PinkerError::Semantic {
                    msg: format!(
                        "parâmetro '{}' do método '{}' no trato '{}' não possui representação nativa válida: {}",
                        param.name, method.name, trait_decl.name, error
                    ),
                    span: param.span,
                })?;
                if !ir_type.is_native_abi_word() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "parâmetro '{}' do método '{}' no trato '{}' exige representação multi-palavra sem transporte nativo nesta fase",
                            param.name, method.name, trait_decl.name
                        ),
                        span: param.span,
                    });
                }
            }

            if let Some(ret_type) = &method.ret_type {
                if Self::type_contains_contextual_self(ret_type) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "método '{}' do trato '{}' não pode retornar 'si' em objeto de trato nesta fase",
                            method.name, trait_decl.name
                        ),
                        span: ret_type.span(),
                    });
                }
            }
        }

        Ok(true)
    }

    fn validate_impl_trait_method_function(
        &self,
        trait_decl: &TraitDecl,
        method: &TraitMethodSig,
        meta: &ImplMethodMeta,
        function: &FunctionDecl,
    ) -> Result<(), PinkerError> {
        if function.params.len() != method.params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' do trato '{}' espera {} parâmetro(s), mas impl para '{}' tem {}",
                    method.name,
                    trait_decl.name,
                    method.params.len(),
                    meta.target_spelling,
                    function.params.len()
                ),
                span: function.span,
            });
        }

        let Some(receiver) = function.params.first() else {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' do trato '{}' exige receiver no impl para '{}'",
                    method.name, trait_decl.name, meta.target_spelling
                ),
                span: function.span,
            });
        };

        let receiver_direct = Self::type_key(&receiver.ty);
        let receiver_identity = self.resolved_type_identity(&receiver.ty)?;

        if meta.identity.target != receiver_identity {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "receiver do método '{}' no impl '{}' para '{}' usa '{}'",
                    method.name, trait_decl.name, meta.target_spelling, receiver_direct
                ),
                span: receiver.span,
            });
        }

        let expected_receiver = method
            .params
            .first()
            .expect("aridade já foi comparada e impl possui receiver");
        if !Self::is_contextual_self_type(&expected_receiver.ty) {
            let expected_ty = self.resolve_type_or_error(&expected_receiver.ty)?;
            let found_ty = self.resolve_type_or_error(&receiver.ty)?;
            if union_canon::canonical_type_key(&expected_ty)
                != union_canon::canonical_type_key(&found_ty)
            {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "receiver do método '{}' no trato '{}' espera '{}', mas impl para '{}' usa '{}'",
                        method.name,
                        trait_decl.name,
                        Self::type_key(&expected_ty),
                        meta.target_spelling,
                        Self::type_key(&found_ty)
                    ),
                    span: receiver.span,
                });
            }
        }

        for (expected, found) in method
            .params
            .iter()
            .skip(1)
            .zip(function.params.iter().skip(1))
        {
            let expected_ty = self.resolve_type_or_error(&expected.ty)?;
            let found_ty = self.resolve_type_or_error(&found.ty)?;

            if union_canon::canonical_type_key(&expected_ty)
                != union_canon::canonical_type_key(&found_ty)
            {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "parâmetro '{}' do método '{}' no trato '{}' espera '{}', mas impl para '{}' usa '{}'",
                        expected.name,
                        method.name,
                        trait_decl.name,
                        Self::type_key(&expected_ty),
                        meta.target_spelling,
                        Self::type_key(&found_ty)
                    ),
                    span: found.span,
                });
            }
        }

        match (&method.ret_type, &function.ret_type) {
            (None, None) => {}
            (Some(expected), Some(found)) => {
                let expected_ty = self.resolve_type_or_error(expected)?;
                let found_ty = self.resolve_type_or_error(found)?;

                if union_canon::canonical_type_key(&expected_ty)
                    != union_canon::canonical_type_key(&found_ty)
                {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "retorno do método '{}' no trato '{}' espera '{}', mas impl para '{}' usa '{}'",
                            method.name,
                            trait_decl.name,
                            Self::type_key(&expected_ty),
                            meta.target_spelling,
                            Self::type_key(&found_ty)
                        ),
                        span: found.span(),
                    });
                }
            }
            _ => {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "retorno do método '{}' no trato '{}' é incompatível no impl para '{}'",
                        method.name, trait_decl.name, meta.target_spelling
                    ),
                    span: function.span,
                });
            }
        }

        Ok(())
    }

    pub(super) fn validate_trait_contracts(&self) -> Result<(), PinkerError> {
        for trait_decl in self.traits.values() {
            if trait_decl.methods.is_empty() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "trato '{}' deve declarar ao menos um método",
                        trait_decl.name
                    ),
                    span: trait_decl.span,
                });
            }

            let objectifiable = self.validate_object_trait_shape(trait_decl)?;

            for method in &trait_decl.methods {
                if objectifiable {
                    let candidates: Vec<(&ImplMethodMeta, &FunctionDecl)> = self
                        .impl_methods
                        .iter()
                        .filter(|meta| {
                            meta.identity.trait_name == trait_decl.name
                                && meta.identity.method_name == method.name
                        })
                        .filter_map(|meta| {
                            self.funcs
                                .get(&meta.function_name)
                                .map(|function| (meta, function))
                        })
                        .collect();

                    if candidates.is_empty() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "trato objetificável '{}' exige ao menos um impl completo para o método '{}'",
                                trait_decl.name, method.name
                            ),
                            span: method.span,
                        });
                    }

                    for (meta, function) in candidates {
                        self.validate_impl_trait_method_function(
                            trait_decl, method, meta, function,
                        )?;
                    }

                    continue;
                }

                let mut candidates = Vec::new();

                if let Some(function) = self.funcs.get(&method.name) {
                    candidates.push(function);
                }

                for meta in &self.impl_methods {
                    if meta.identity.trait_name == trait_decl.name
                        && meta.identity.method_name == method.name
                    {
                        if let Some(function) = self.funcs.get(&meta.function_name) {
                            candidates.push(function);
                        }
                    }
                }

                if candidates.is_empty() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "trato '{}' exige função '{}' compatível declarada no topo",
                            trait_decl.name, method.name
                        ),
                        span: method.span,
                    });
                }

                let mut first_error = None;

                for function in candidates {
                    match self.validate_trait_method_function(trait_decl, method, function) {
                        Ok(()) => {
                            first_error = None;
                            break;
                        }
                        Err(err) if first_error.is_none() => first_error = Some(err),
                        Err(_) => {}
                    }
                }

                if let Some(err) = first_error {
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    fn validate_trait_method_function(
        &self,
        trait_decl: &TraitDecl,
        method: &TraitMethodSig,
        function: &FunctionDecl,
    ) -> Result<(), PinkerError> {
        if function.params.len() != method.params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' do trato '{}' espera {} parâmetro(s), mas função declarada tem {}",
                    method.name,
                    trait_decl.name,
                    method.params.len(),
                    function.params.len()
                ),
                span: function.span,
            });
        }
        for (expected, found) in method.params.iter().zip(function.params.iter()) {
            let expected_ty = self.resolve_type_or_error(&expected.ty)?;
            let found_ty = self.resolve_type_or_error(&found.ty)?;
            if Self::type_key(&expected_ty) != Self::type_key(&found_ty) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "parâmetro '{}' do método '{}' no trato '{}' espera '{}', mas função usa '{}'",
                        expected.name,
                        method.name,
                        trait_decl.name,
                        Self::type_key(&expected_ty),
                        Self::type_key(&found_ty)
                    ),
                    span: found.span,
                });
            }
        }
        match (&method.ret_type, &function.ret_type) {
            (None, None) => {}
            (Some(expected), Some(found)) => {
                let expected_ty = self.resolve_type_or_error(expected)?;
                let found_ty = self.resolve_type_or_error(found)?;
                if Self::type_key(&expected_ty) != Self::type_key(&found_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "retorno do método '{}' no trato '{}' espera '{}', mas função usa '{}'",
                            method.name,
                            trait_decl.name,
                            Self::type_key(&expected_ty),
                            Self::type_key(&found_ty)
                        ),
                        span: found.span(),
                    });
                }
            }
            _ => {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "retorno do método '{}' no trato '{}' é incompatível com a função declarada",
                        method.name, trait_decl.name
                    ),
                    span: function.span,
                });
            }
        }
        Ok(())
    }
    // @pinker-nav:end semantic.tratos.contratos
}
