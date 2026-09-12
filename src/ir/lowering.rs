//! Lowering de funções, blocos, comandos, expressões, bindings e constantes da
//! IR estruturada, movido de `src/ir.rs` pela unidade IR-1 do inventário da
//! #601 (Task #621).
//!
//! Só o arquivo mudou: o `impl FunctionLowerer` inteiro — as cinco regiões
//! cartografadas `ir.lowering.funcoes-blocos`, `ir.lowering.comandos-controle`,
//! `ir.lowering.expressoes-valores`, `ir.lowering.bindings-escopos` e
//! `ir.lowering.constantes` —, os quarenta e seis corpos que as compõem e a
//! ordem em que decidem continuam exatamente como estavam. `super` mudou de
//! significado ao descer um nível, e o `use` abaixo devolve ao irmão o
//! vocabulário do pai — `FunctionLowerer`, `LoweringContext`, os tipos da AST,
//! os modelos `ProgramIR`/`FunctionIR`/`InstructionIR`/`ValueIR`/`TypeIR` e os
//! helpers privados — sem promover nada: um filho enxerga os itens privados do
//! pai por privacidade de módulo, e este `use` é privado.
//!
//! O estado continua sendo do pai: `struct FunctionLowerer` e
//! `struct LoweringContext` seguem declarados em `src/ir.rs`, com os mesmos
//! campos privados. Este arquivo é implementação física da mesma fase de
//! lowering, não uma fase nova.
//!
//! A autoridade da seleção de método continua sendo `crate::method_dispatch`
//! (#590/#591, consolidação C2). `resolve_impl_method` só constrói candidatos a
//! partir da visão derivada e traduz o veredito; nenhuma regra de precedência,
//! desempate, escolha de representante ou decisão por grafia nasce aqui. A
//! validação da IR continua em `src/ir_validate.rs` e a fronteira de CFG
//! continua em `src/cfg_ir.rs`; nada disso desceu com o corte.
//!
//! Nenhum item do corte era `pub` antes do move e nenhum é agora. `new`,
//! `lower_function` e `lower_const` são os três símbolos que o pai chama e, por
//! isso, os únicos que passaram de privados a `pub(super)`.

use super::*;

impl<'a> FunctionLowerer<'a> {
    // @pinker-nav:start ir.lowering.funcoes-blocos
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Configuração do `FunctionLowerer` e lowering de funções/blocos estruturados: aloca parâmetros e preserva metadados nominais/estruturais de callables, ponteiros crus e pointees de ponteiros de dados em aliases, retornos, ternários, chamadas por expressão e capturas de closure. Inclui resolvedores de método de `impl` direto e qualificado por trato; o direto só constrói candidatos da visão derivada e delega o veredito a `method_dispatch`, a mesma autoridade que a semântica consulta, e o qualificado nomeia o trato e continua sendo consulta de identidade, correspondida desde a #647 por `method_identity`, autoridade única das três componentes da identidade, com a IR trazendo só o índice e a tradução para `Option`; preserva a estrutura aninhada, sem ainda dividir o fluxo em CFG.
    pub(super) fn new(context: &'a LoweringContext) -> Self {
        Self {
            context,
            scopes: Vec::new(),
            params: Vec::new(),
            locals: Vec::new(),
            slot_counters: HashMap::new(),
            block_counter: 0,
            loop_exit_stack: Vec::new(),
            loop_continue_stack: Vec::new(),
            callable_metadata: HashMap::new(),
            raw_function_metadata: HashMap::new(),
            pointer_pointee_types: HashMap::new(),
            trait_object_names: HashMap::new(),
        }
    }

    /// Nome nominal do receptor de um `impl`, consultado na tabela de
    /// identidades. O nome nunca é autoridade de identidade — é apenas a chave
    /// textual do catálogo de métodos, derivada da identidade resolvida.
    fn impl_receiver_key(&self, typed: &TypedValueIR) -> Option<String> {
        if typed.ty == TypeIR::Struct {
            return self.nominal_name_of_value(typed);
        }
        Some(typed.ty.name().to_string())
    }

    /// Identidade semântica de um nome de função usado como valor.
    ///
    /// Deriva da assinatura declarada (`carinho(P...) -> R`), não do nome do
    /// símbolo nem do wrapper sintético de `__env`.
    fn function_value_identity(
        &self,
        name: &str,
        span: Span,
    ) -> Result<Option<ResolvedTypeId>, PinkerError> {
        let Some(declaration) = self.context.all_functions.get(name) else {
            return Ok(None);
        };
        let params = declaration
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect::<Vec<_>>();
        let ret = declaration
            .ret_type
            .clone()
            .unwrap_or_else(|| Type::Nulo(span));
        let identity = self.context.resolved_identity(&Type::Function {
            params,
            ret: Box::new(ret),
            span,
        })?;
        Ok(Some(identity))
    }

    /// Identidade semântica do valor devolvido por uma chamada indireta.
    ///
    /// `trato<Nome>` é reconstruído a partir do nome nominal que a metadata do
    /// callable já preserva; nas demais representações a identidade é a própria
    /// representação e fica `None` (resolvida sob demanda).
    fn callable_ret_identity(
        &self,
        metadata: &CallableMetadata,
        span: Span,
    ) -> Result<Option<ResolvedTypeId>, PinkerError> {
        // O caminho normal é o tipo AST do retorno declarado: é ele que resolve
        // apelidos e distingue dois `leque` de mesma representação.
        if let Some(ret_ast) = metadata.ret_ast.as_ref() {
            return Ok(Some(self.context.resolved_identity(ret_ast)?));
        }
        if metadata.ret_type != TypeIR::TraitObject {
            return Ok(None);
        }
        let Some(trait_name) = metadata.ret_trait_name.as_ref() else {
            return Err(PinkerError::Ir {
                msg: "E-IR-TYPE-IDENTITY-LOST: retorno 'trato' de callable sem nome nominal"
                    .to_string(),
                span,
            });
        };
        let identity = self.context.resolved_identity(&Type::Applied {
            name: "trato".to_string(),
            args: vec![Type::Alias {
                name: trait_name.clone(),
                span,
            }],
            span,
        })?;
        Ok(Some(identity))
    }

    /// Identidade semântica do valor devolvido por uma chamada por ponteiro cru.
    fn raw_ret_identity(
        &self,
        metadata: &RawFunctionMetadata,
        span: Span,
    ) -> Result<Option<ResolvedTypeId>, PinkerError> {
        let _ = span;
        match metadata.ret_ast.as_ref() {
            Some(ret_ast) => Ok(Some(self.context.resolved_identity(ret_ast)?)),
            None => Ok(None),
        }
    }

    /// Identidade do apontado de um valor ponteiro, com sua representação.
    fn pointee_identity_of(&self, typed: &TypedValueIR) -> Option<(ResolvedTypeId, TypeIR)> {
        let resolved = typed.resolved?;
        let table = self.context.resolved_types.borrow();
        let pointee = table.get(resolved)?.pointee?;
        let representation = table.get(pointee)?.representation;
        Some((pointee, representation))
    }

    fn pointer_element_layout(
        &self,
        pointer: &TypedValueIR,
        span: Span,
    ) -> Result<layout::TypeLayout, PinkerError> {
        let (pointee, representation) =
            self.pointee_identity_of(pointer)
                .ok_or_else(|| PinkerError::Ir {
                    msg: "E-IR-POINTER-LAYOUT: identidade do elemento de seta<T> foi perdida"
                        .to_string(),
                    span,
                })?;
        let nominal_name = self
            .context
            .resolved_types
            .borrow()
            .get(pointee)
            .and_then(|entry| entry.nominal_name.clone());
        let element = match representation {
            TypeIR::Bombom => Type::Bombom(span),
            TypeIR::U8 => Type::U8(span),
            TypeIR::U16 => Type::U16(span),
            TypeIR::U32 => Type::U32(span),
            TypeIR::U64 => Type::U64(span),
            TypeIR::I8 => Type::I8(span),
            TypeIR::I16 => Type::I16(span),
            TypeIR::I32 => Type::I32(span),
            TypeIR::I64 => Type::I64(span),
            TypeIR::Logica => Type::Logica(span),
            TypeIR::FixedArray {
                element: ScalarTypeIR::Bombom,
                size,
            } => Type::FixedArray {
                element: Box::new(Type::Bombom(span)),
                size,
                span,
            },
            TypeIR::Struct => Type::Struct {
                name: nominal_name.ok_or_else(|| PinkerError::Ir {
                    msg: "E-IR-POINTER-LAYOUT: identidade nominal do ninho apontado foi perdida"
                        .to_string(),
                    span,
                })?,
                span,
            },
            _ => {
                return Err(PinkerError::Ir {
                    msg: format!(
                        "E-IR-POINTER-LAYOUT: tipo '{}' não participa da aritmética D5",
                        representation.name()
                    ),
                    span,
                })
            }
        };
        layout::layout_of_type(
            &element,
            &self.context.type_aliases,
            &self.context.struct_decls,
        )
        .map_err(|msg| PinkerError::Ir {
            msg: format!("E-IR-POINTER-LAYOUT: {}", msg),
            span,
        })
    }

    /// Nome nominal (`ninho`/`leque`) da identidade de um valor, se houver.
    fn nominal_name_of_value(&self, typed: &TypedValueIR) -> Option<String> {
        let resolved = typed.resolved?;
        self.context
            .resolved_types
            .borrow()
            .nominal_name_of(resolved)
            .map(str::to_string)
    }

    fn callable_metadata_from_return_type(
        &self,
        ret: &Type,
    ) -> Result<CallableMetadata, PinkerError> {
        Ok(CallableMetadata {
            ret_type: self.context.resolve_type(ret)?,
            ret_ast: Some(ret.clone()),
            ret_trait_name: trait_object_name_from_type(
                ret,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            ret_pointer_pointee: pointer_pointee_from_type(
                ret,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
        })
    }

    fn callable_metadata_for_value(&self, value: &ValueIR) -> Option<CallableMetadata> {
        match value {
            ValueIR::FunctionRef(name) => self
                .context
                .closure_state
                .borrow()
                .wrapper_metadata
                .get(name)
                .cloned(),
            ValueIR::MakeClosure { function_name, .. } => self
                .context
                .function_sigs
                .get(function_name)
                .map(|sig| CallableMetadata {
                    ret_type: sig.ret_type,
                    ret_ast: self
                        .context
                        .all_functions
                        .get(function_name)
                        .and_then(|declaration| declaration.ret_type.clone()),
                    ret_trait_name: self
                        .context
                        .function_ret_trait_names
                        .get(function_name)
                        .cloned(),
                    ret_pointer_pointee: self
                        .context
                        .function_ret_pointer_pointees
                        .get(function_name)
                        .copied(),
                }),
            ValueIR::Local(slot) => self.callable_metadata.get(slot).cloned(),
            ValueIR::Call { callee, .. } => self.context.callable_metadata.get(callee).cloned(),
            _ => None,
        }
    }

    fn raw_function_metadata_for_value(
        &self,
        value: &ValueIR,
    ) -> Result<Option<RawFunctionMetadata>, PinkerError> {
        match value {
            ValueIR::RawFunctionRef(name) => self
                .context
                .all_functions
                .get(name)
                .map(|function| {
                    raw_function_metadata_from_decl(
                        function,
                        &self.context.type_aliases,
                        &self.context.struct_names,
                    )
                })
                .transpose(),
            ValueIR::Local(slot) => Ok(self.raw_function_metadata.get(slot).cloned()),
            ValueIR::Call { callee, args, .. } if callee == "__ternario" => {
                let [_, true_value, false_value] = args.as_slice() else {
                    return Ok(None);
                };
                let true_metadata = self.raw_function_metadata_for_value(true_value)?;
                let false_metadata = self.raw_function_metadata_for_value(false_value)?;
                Ok(match (true_metadata, false_metadata) {
                    (Some(true_metadata), Some(false_metadata))
                        if true_metadata == false_metadata =>
                    {
                        Some(true_metadata)
                    }
                    _ => None,
                })
            }
            ValueIR::Call { callee, .. } => Ok(self
                .context
                .raw_function_return_metadata
                .get(callee)
                .cloned()),
            _ => Ok(None),
        }
    }

    fn pointer_pointee_for_expr(&self, expr: &Expr) -> Result<Option<TypeIR>, PinkerError> {
        match &expr.kind {
            ExprKind::Ident(name) => Ok(self
                .resolve_existing_binding(name)
                .and_then(|binding| self.pointer_pointee_types.get(&binding.slot).copied())),
            ExprKind::Cast { target, .. } => pointer_pointee_from_type(
                target,
                &self.context.type_aliases,
                &self.context.struct_names,
            ),
            ExprKind::Call(callee, _) => {
                if let ExprKind::Ident(name) = &callee.kind {
                    if name == "alocar" {
                        return Ok(Some(TypeIR::U8));
                    }
                    if let Some(binding) = self.resolve_existing_binding(name) {
                        if let Some(pointee) = self
                            .raw_function_metadata
                            .get(&binding.slot)
                            .and_then(|metadata| metadata.ret_pointer_pointee)
                        {
                            return Ok(Some(pointee));
                        }
                        if let Some(pointee) = self
                            .callable_metadata
                            .get(&binding.slot)
                            .and_then(|metadata| metadata.ret_pointer_pointee)
                        {
                            return Ok(Some(pointee));
                        }
                    }
                    return Ok(self
                        .context
                        .function_ret_pointer_pointees
                        .get(name)
                        .copied());
                }
                Ok(self
                    .raw_function_metadata_for_expr(callee)?
                    .and_then(|metadata| metadata.ret_pointer_pointee))
            }
            ExprKind::Binary(lhs, BinaryOp::Add | BinaryOp::Sub, rhs) => {
                let left = self.pointer_pointee_for_expr(lhs)?;
                if left.is_some() {
                    Ok(left)
                } else {
                    self.pointer_pointee_for_expr(rhs)
                }
            }
            _ => Ok(None),
        }
    }

    fn raw_function_metadata_for_expr(
        &self,
        expr: &Expr,
    ) -> Result<Option<RawFunctionMetadata>, PinkerError> {
        match &expr.kind {
            ExprKind::Ident(name) => Ok(self
                .resolve_existing_binding(name)
                .and_then(|binding| self.raw_function_metadata.get(&binding.slot).cloned())),
            ExprKind::AddressOf(operand) => {
                let ExprKind::Ident(name) = &operand.kind else {
                    return Ok(None);
                };
                self.context
                    .all_functions
                    .get(name)
                    .map(|function| {
                        raw_function_metadata_from_decl(
                            function,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )
                    })
                    .transpose()
            }
            ExprKind::Call(callee, args) if matches!(&callee.kind, ExprKind::Ident(name) if name == "__ternario") =>
            {
                let [_, true_value, false_value] = args.as_slice() else {
                    return Ok(None);
                };
                let true_metadata = self.raw_function_metadata_for_expr(true_value)?;
                let false_metadata = self.raw_function_metadata_for_expr(false_value)?;
                Ok(match (true_metadata, false_metadata) {
                    (Some(true_metadata), Some(false_metadata))
                        if true_metadata == false_metadata =>
                    {
                        Some(true_metadata)
                    }
                    _ => None,
                })
            }
            _ => Ok(None),
        }
    }

    fn callable_metadata_for_expr(
        &self,
        expr: &Expr,
    ) -> Result<Option<CallableMetadata>, PinkerError> {
        match &expr.kind {
            ExprKind::Ident(name) => {
                if let Some(binding) = self.resolve_existing_binding(name) {
                    return Ok((binding.ty == TypeIR::Function)
                        .then(|| self.callable_metadata.get(&binding.slot).cloned())
                        .flatten());
                }

                Ok(self
                    .context
                    .function_sigs
                    .get(name)
                    .map(|sig| CallableMetadata {
                        ret_type: sig.ret_type,
                        ret_ast: self
                            .context
                            .all_functions
                            .get(name)
                            .and_then(|declaration| declaration.ret_type.clone()),
                        ret_trait_name: self.context.function_ret_trait_names.get(name).cloned(),
                        ret_pointer_pointee: self
                            .context
                            .function_ret_pointer_pointees
                            .get(name)
                            .copied(),
                    }))
            }
            ExprKind::Call(callee, args) => {
                let ExprKind::Ident(name) = &callee.kind else {
                    return Ok(None);
                };
                if name != "__ternario" {
                    return Ok(self.context.callable_metadata.get(name).cloned());
                }

                let [_, true_value, false_value] = args.as_slice() else {
                    return Err(PinkerError::Ir {
                        msg: "lowering encontrou ternário callable sem três argumentos".to_string(),
                        span: expr.span,
                    });
                };
                let true_metadata = self.callable_metadata_for_expr(true_value)?;
                let false_metadata = self.callable_metadata_for_expr(false_value)?;
                let (Some(true_metadata), Some(false_metadata)) = (true_metadata, false_metadata)
                else {
                    return Err(PinkerError::Ir {
                        msg: "ternário callable exige metadados nos dois braços".to_string(),
                        span: expr.span,
                    });
                };
                if true_metadata.ret_type != false_metadata.ret_type {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "ternário callable possui retornos estruturais incompatíveis: {} e {}",
                            true_metadata.ret_type.name(),
                            false_metadata.ret_type.name()
                        ),
                        span: expr.span,
                    });
                }
                if true_metadata.ret_trait_name != false_metadata.ret_trait_name {
                    return Err(PinkerError::Ir {
                        msg: "ternário callable possui retornos nominais incompatíveis".to_string(),
                        span: expr.span,
                    });
                }

                Ok(Some(true_metadata))
            }
            _ => Ok(None),
        }
    }

    fn resolve_impl_method(
        &self,
        receiver: &TypedValueIR,
        method_name: &str,
        span: Span,
    ) -> Result<Option<String>, PinkerError> {
        let receiver_identity = receiver.identity(self.context, span)?;
        // Esta fase só constrói candidatos a partir da sua visão derivada e
        // nomeia a relação de cada um. Quem alcança, quem precede e quem vence
        // é `method_dispatch`, a mesma autoridade que a semântica consultou;
        // sem isso, `--check` e o lowering discordariam sobre o mesmo programa.
        let candidates = self
            .context
            .impl_methods
            .iter()
            .filter(|(identity, _function_name)| {
                identity.target == receiver_identity && identity.method_name == method_name
            })
            .map(|(identity, function_name)| DispatchCandidate {
                function_name: function_name.clone(),
                relation: Some(DispatchRelation {
                    trait_name: identity.trait_name.clone(),
                    fonte_da_relacao: self.fonte_da_relacao(&identity.trait_name, identity.target),
                }),
            });
        match method_dispatch::select_impl_method(
            &self.context.traits_visiveis_por_fonte,
            span,
            candidates,
        ) {
            MethodSelection::Winner(function_name) => Ok(Some(function_name)),
            MethodSelection::NoMatch | MethodSelection::Ambiguous => Ok(None),
        }
    }

    /// Unidade que DECLAROU a relação `(trato canônico, alvo resolvido)`.
    ///
    /// Adaptador único do lowering para a proveniência de relação; a pergunta
    /// de alcance em si é de `module_resolve`.
    fn fonte_da_relacao(&self, trait_name: &str, target: ResolvedTypeId) -> Option<SourceId> {
        self.context
            .fontes_das_relacoes
            .get(&(trait_name.to_string(), target))
            .copied()
    }

    /// A relação `(trato, alvo)` alcança quem escreveu `span`?
    ///
    /// #649/`POLICY_B_RELATION_REACHABILITY_ALWAYS_MATTERS` — a autoridade é
    /// `module_resolve`, a MESMA que a checagem semântica e
    /// o despacho não qualificado consultam. Sem isso `--check` e o lowering
    /// discordariam sobre o mesmo programa.
    fn relacao_alcanca(&self, trait_name: &str, target: ResolvedTypeId, span: Span) -> bool {
        crate::module_resolve::relacao_alcanca(
            &self.context.traits_visiveis_por_fonte,
            span,
            self.fonte_da_relacao(trait_name, target),
        )
    }

    fn resolve_qualified_impl_method(
        &self,
        receiver: &TypedValueIR,
        trait_name: &str,
        method_name: &str,
        span: Span,
    ) -> Result<Option<String>, PinkerError> {
        // #647/U-03A: a correspondência é de `method_identity`, a mesma que a
        // checagem semântica consulta. Aqui só sobram o adaptador de
        // representação — o receiver da IR vira identidade resolvida — e a
        // tradução do veredito para `Option`, que é do lowering.
        let receiver_identity = receiver.identity(self.context, span)?;
        let function_name = match self.resolve_qualified_impl_method_symbol(
            trait_name,
            &receiver_identity,
            method_name,
        ) {
            QualifiedMethodResolution::Resolved(function_name) => function_name,
            QualifiedMethodResolution::NoMatch => return Ok(None),
        };
        // #649 — identidade exata não autoriza. Primeiro QUAL relação, depois
        // se ESTE contexto pode usá-la: a chamada qualificada deixa de ser
        // bypass da política modular, e a ordem das duas perguntas é a mesma
        // que a checagem semântica aplica.
        if !self.relacao_alcanca(trait_name, receiver_identity, span) {
            return Ok(None);
        }
        Ok(Some(function_name))
    }

    /// Adaptador único do índice da IR para a autoridade qualificada.
    fn resolve_qualified_impl_method_symbol(
        &self,
        trait_name: &str,
        target: &ResolvedTypeId,
        method_name: &str,
    ) -> QualifiedMethodResolution {
        method_identity::resolve_qualified_impl_method(
            self.context
                .impl_methods
                .iter()
                .map(|(identity, function_name)| (identity, function_name.as_str())),
            trait_name,
            target,
            method_name,
        )
    }

    fn resolve_trait_impl_symbol(
        &self,
        trait_name: &str,
        target: ResolvedTypeId,
        method_name: &str,
    ) -> Option<String> {
        match self.resolve_qualified_impl_method_symbol(trait_name, &target, method_name) {
            QualifiedMethodResolution::Resolved(function_name) => Some(function_name),
            QualifiedMethodResolution::NoMatch => None,
        }
    }

    fn trait_object_name_for_expr(&self, expr: &Expr) -> Result<Option<String>, PinkerError> {
        let trait_name = match &expr.kind {
            ExprKind::Ident(name) => {
                let Some(binding) = self.resolve_existing_binding(name) else {
                    return Ok(None);
                };

                self.trait_object_names.get(&binding.slot).cloned()
            }
            ExprKind::Cast { target, .. } => trait_object_name_from_type(
                target,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            ExprKind::Call(callee, args) => match &callee.kind {
                ExprKind::Ident(function_name) if function_name == "__ternario" => {
                    let [_, true_value, false_value] = args.as_slice() else {
                        return Err(PinkerError::Ir {
                            msg: "lowering encontrou ternário sem três argumentos".to_string(),
                            span: expr.span,
                        });
                    };
                    let true_trait = self.trait_object_name_for_expr(true_value)?;
                    let false_trait = self.trait_object_name_for_expr(false_value)?;
                    match (true_trait, false_trait) {
                        (Some(true_trait), Some(false_trait)) if true_trait == false_trait => {
                            Some(true_trait)
                        }
                        (None, None) => None,
                        (Some(true_trait), Some(false_trait)) => {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "ternário perdeu compatibilidade nominal entre trato<{}> e trato<{}>",
                                    true_trait, false_trait
                                ),
                                span: expr.span,
                            });
                        }
                        _ => {
                            return Err(PinkerError::Ir {
                                msg: "ternário de objeto de trato exige identidade nominal nos dois braços"
                                    .to_string(),
                                span: expr.span,
                            });
                        }
                    }
                }
                ExprKind::Ident(function_name) => {
                    if let Some(binding) = self.resolve_existing_binding(function_name) {
                        if binding.ty == TypeIR::Function {
                            let metadata =
                                self.callable_metadata.get(&binding.slot).ok_or_else(|| {
                                    PinkerError::Ir {
                                        msg: format!(
                                            "lowering perdeu os metadados do callable '{}'",
                                            function_name
                                        ),
                                        span: expr.span,
                                    }
                                })?;
                            if metadata.ret_type == TypeIR::TraitObject
                                && metadata.ret_trait_name.is_none()
                            {
                                return Err(PinkerError::Ir {
                                    msg: format!(
                                        "lowering perdeu a identidade nominal do trato retornado pelo callable '{}'",
                                        function_name
                                    ),
                                    span: expr.span,
                                });
                            }
                            metadata.ret_trait_name.clone()
                        } else {
                            None
                        }
                    } else {
                        self.context
                            .function_ret_trait_names
                            .get(function_name)
                            .cloned()
                    }
                }
                ExprKind::FieldAccess { base, field } => {
                    let trait_name = match &base.kind {
                        ExprKind::Ident(name) if self.context.traits.contains_key(name) => {
                            name.clone()
                        }
                        _ => {
                            let Some(name) = self.trait_object_name_for_expr(base)? else {
                                return Ok(None);
                            };
                            name
                        }
                    };
                    self.context
                        .traits
                        .get(&trait_name)
                        .and_then(|meta| meta.methods.iter().find(|method| method.name == *field))
                        .and_then(|method| method.ret_trait_name.clone())
                }
                _ => None,
            },
            _ => None,
        };
        Ok(trait_name)
    }

    fn concrete_snapshot_size(&self, value: &TypedValueIR, span: Span) -> Result<u64, PinkerError> {
        match value.ty {
            TypeIR::Bombom | TypeIR::U64 | TypeIR::I64 => Ok(8),
            TypeIR::U32 | TypeIR::I32 => Ok(4),
            TypeIR::U16 | TypeIR::I16 => Ok(2),
            TypeIR::U8 | TypeIR::I8 | TypeIR::Logica => Ok(1),

            // Categorias representadas por handle ou ponteiro de uma palavra.
            TypeIR::Verso
            | TypeIR::ListBombom
            | TypeIR::ListVerso
            | TypeIR::MapVersoBombom
            | TypeIR::MapVersoVerso
            | TypeIR::MapBombomBombom
            | TypeIR::MapBombomVerso
            | TypeIR::Map { .. }
            | TypeIR::OpaqueWordHandle
            | TypeIR::Pointer { .. }
            | TypeIR::Function
            | TypeIR::Union(_)
            | TypeIR::FunctionPointer => Ok(layout::POINTER_SIZE),

            TypeIR::FixedArray { element, size } => {
                let element_size: u64 = match element {
                    ScalarTypeIR::Bombom | ScalarTypeIR::U64 | ScalarTypeIR::I64 => 8,
                    ScalarTypeIR::U32 | ScalarTypeIR::I32 => 4,
                    ScalarTypeIR::U16 | ScalarTypeIR::I16 => 2,
                    ScalarTypeIR::U8 | ScalarTypeIR::I8 | ScalarTypeIR::Logica => 1,
                };

                element_size
                    .checked_mul(size)
                    .ok_or_else(|| PinkerError::Ir {
                        msg: "overflow ao calcular snapshot de array".to_string(),
                        span,
                    })
            }

            TypeIR::Struct => {
                let struct_name =
                    self.nominal_name_of_value(value)
                        .ok_or_else(|| PinkerError::Ir {
                            msg: "E-IR-TYPE-IDENTITY-LOST: snapshot de ninho sem identidade \
                                  resolvida"
                                .to_string(),
                            span,
                        })?;

                let ast_type = Type::Struct {
                    name: struct_name,
                    span,
                };

                layout::layout_of_type(
                    &ast_type,
                    &self.context.type_aliases,
                    &self.context.struct_decls,
                )
                .map(|layout| layout.size)
                .map_err(|msg| PinkerError::Ir {
                    msg: format!("layout inválido para snapshot do objeto de trato: {}", msg),
                    span,
                })
            }

            TypeIR::TraitObject | TypeIR::Nulo => Err(PinkerError::Ir {
                msg: "tipo concreto inválido para materialização de objeto de trato".to_string(),
                span,
            }),
        }
    }

    fn trait_vtable(
        &self,
        trait_name: &str,
        target: ResolvedTypeId,
        target_display: &str,
        span: Span,
    ) -> Result<Vec<String>, PinkerError> {
        // #649 — a relação concreta entra na representação dinâmica AQUI, uma
        // vez por objeto formado. O alcance é perguntado neste ponto e em
        // nenhum outro: `lower_trait_call` consome o slot já autorizado e não
        // refaz busca modular a cada chamada dinâmica.
        if !self.relacao_alcanca(trait_name, target, span) {
            return Err(PinkerError::Ir {
                msg: format!(
                    "impl de '{}' para '{}' não é alcançável desta unidade e não pode formar objeto de trato",
                    trait_name, target_display
                ),
                span,
            });
        }
        let trait_meta = self
            .context
            .traits
            .get(trait_name)
            .ok_or_else(|| PinkerError::Ir {
                msg: format!("lowering não encontrou metadados do trato '{}'", trait_name),
                span,
            })?;

        trait_meta
            .methods
            .iter()
            .map(|method| {
                self.resolve_trait_impl_symbol(trait_name, target, &method.name)
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!(
                            "lowering não encontrou impl de '{}.{}' para '{}'",
                            trait_name, method.name, target_display
                        ),
                        span,
                    })
            })
            .collect()
    }

    fn lower_trait_call(
        &mut self,
        object: TypedValueIR,
        trait_name: &str,
        method_name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<TypedValueIR, PinkerError> {
        let (method_slot, method_count, method) = {
            let trait_meta =
                self.context
                    .traits
                    .get(trait_name)
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!("lowering não encontrou metadados do trato '{}'", trait_name),
                        span,
                    })?;

            let (slot, method) = trait_meta
                .methods
                .iter()
                .enumerate()
                .find(|(_, method)| method.name == method_name)
                .ok_or_else(|| PinkerError::Ir {
                    msg: format!(
                        "lowering não encontrou método '{}.{}'",
                        trait_name, method_name
                    ),
                    span,
                })?;

            (slot as u64, trait_meta.methods.len() as u64, method.clone())
        };

        if args.len() != method.param_types.len() {
            return Err(PinkerError::Ir {
                msg: format!(
                    "lowering recebeu aridade inconsistente em '{}.{}'",
                    trait_name, method_name
                ),
                span,
            });
        }

        let lowered_args = args
            .iter()
            .map(|arg| self.lower_value(arg).map(|typed| typed.value))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TypedValueIR {
            value: ValueIR::TraitCall {
                object: Box::new(object.value),
                trait_name: trait_name.to_string(),
                method_name: method_name.to_string(),
                method_slot,
                method_count,
                args: lowered_args,
                param_types: method.param_types,
                ret_type: method.ret_type,
            },
            ty: method.ret_type,
            resolved: method
                .ret_ast
                .as_ref()
                .map(|ty| self.context.resolved_identity(ty))
                .transpose()?,
            ptr_array_bombom_size: None,
        })
    }

    pub(super) fn lower_function(
        mut self,
        function: &FunctionDecl,
    ) -> Result<FunctionIR, PinkerError> {
        self.push_scope();

        for param in &function.params {
            let binding = self.allocate_binding(
                &param.name,
                self.context.resolve_type(&param.ty)?,
                Some(self.context.resolved_identity(&param.ty)?),
                pointer_to_bombom_array_size(&param.ty, &self.context.type_aliases),
                None,
            )?;

            if let Some(trait_name) = trait_object_name_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.trait_object_names
                    .insert(binding.slot.clone(), trait_name);
            }

            if let Type::Function { ret, .. } = &param.ty {
                let ret_ty = self.context.resolve_type(ret)?;
                self.callable_metadata.insert(
                    binding.slot.clone(),
                    CallableMetadata {
                        ret_type: ret_ty,
                        ret_ast: Some(ret.as_ref().clone()),
                        ret_trait_name: trait_object_name_from_type(
                            ret,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )?,
                        ret_pointer_pointee: pointer_pointee_from_type(
                            ret,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )?,
                    },
                );
            }
            if let Some(metadata) = raw_function_metadata_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.raw_function_metadata
                    .insert(binding.slot.clone(), metadata);
            }
            if let Some(pointee) = pointer_pointee_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.pointer_pointee_types
                    .insert(binding.slot.clone(), pointee);
            }
            self.params.push(binding);
        }

        let entry = self.lower_block(&function.body, "entry".to_string(), false)?;

        self.pop_scope();

        Ok(FunctionIR {
            name: function.name.clone(),
            params: self.params,
            locals: self.locals,
            ret_type: TypeIR::from_ast_option_with_context(
                function.ret_type.as_ref(),
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            entry,
            span: function.span,
        })
    }

    // Fase 243: resolve um literal `carinho` no ponto de criação. Espelha
    // `semantic.rs::resolve_closure_value` — mesma varredura sintática
    // (`ast::free_identifiers_in_function`), mesma regra de captura (nome
    // livre que resolve como binding LOCAL nesta função, via
    // `resolve_existing_binding`; os demais não são captura, resolvidos
    // normalmente dentro do próprio corpo da closure). Cada literal tem
    // exatamente um ponto de criação (desaçucaramento da Fase 225 o
    // substitui por um único `Ident`), então esta função nunca é chamada
    // duas vezes para o mesmo nome em um programa válido.
    // Fase 243: gera (memoizado) um wrapper sintético `__fnref_env_<nome>`
    // para uma função top-level usada como valor callable. O wrapper tem a
    // MESMA assinatura pública de `nome`, mais um parâmetro oculto final
    // `__env` (ignorado), e o corpo é só `mimo nome(params...)`. Isso torna
    // a convenção de chamada indireta uniforme (todo callable — closure ou
    // referência a função top-level — aceita `__env` por último) sem tocar
    // em nenhuma chamada direta existente a `nome` (que continua chamando
    // `nome` de verdade, não o wrapper).
    fn ensure_fnref_wrapper(&mut self, name: &str, span: Span) -> Result<String, PinkerError> {
        let wrapper_name = format!("__fnref_env_{}", name);
        if self
            .context
            .closure_state
            .borrow()
            .wrapper_metadata
            .contains_key(&wrapper_name)
        {
            return Ok(wrapper_name);
        }

        let function = self
            .context
            .all_functions
            .get(name)
            .cloned()
            .ok_or_else(|| PinkerError::Ir {
                msg: format!(
                    "lowering falhou ao materializar wrapper de '{}' (função sem corpo AST — provável intrínseca; referenciar intrínsecas como valor não é suportado)",
                    name
                ),
                span,
            })?;

        let mut wrapper = FunctionLowerer::new(self.context);
        wrapper.push_scope();
        let mut call_args = Vec::new();
        let mut wrapper_params = Vec::new();
        for param in &function.params {
            let binding = wrapper.allocate_binding(
                &param.name,
                wrapper.context.resolve_type(&param.ty)?,
                Some(wrapper.context.resolved_identity(&param.ty)?),
                pointer_to_bombom_array_size(&param.ty, &wrapper.context.type_aliases),
                None,
            )?;
            call_args.push(ValueIR::Local(binding.slot.clone()));
            wrapper_params.push(binding);
        }
        let ret_type = TypeIR::from_ast_option_with_context(
            function.ret_type.as_ref(),
            &wrapper.context.type_aliases,
            &wrapper.context.struct_names,
        )?;
        let env_pointer = TypeIR::Pointer { is_volatile: false };
        let env_identity = wrapper
            .context
            .internal_identity("env", env_pointer, function.span)?;
        let env_binding =
            wrapper.allocate_binding("__env", env_pointer, Some(env_identity), None, None)?;
        wrapper_params.push(env_binding);
        wrapper.pop_scope();

        let wrapper_fn = FunctionIR {
            name: wrapper_name.clone(),
            params: wrapper_params,
            locals: Vec::new(),
            ret_type,
            entry: BlockIR {
                label: "entry".to_string(),
                instructions: vec![InstructionIR::Return {
                    value: Some(ValueIR::Call {
                        identidade: crate::intrinsics::identity::CalleeIdentity::User,
                        callee: name.to_string(),
                        args: call_args,
                        ret_type,
                    }),
                    span,
                }],
                span,
            },
            span,
        };

        let mut state = self.context.closure_state.borrow_mut();
        state.wrapper_metadata.insert(
            wrapper_name.clone(),
            CallableMetadata {
                ret_type,
                ret_ast: function.ret_type.clone(),
                ret_trait_name: function
                    .ret_type
                    .as_ref()
                    .map(|ty| {
                        trait_object_name_from_type(
                            ty,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )
                    })
                    .transpose()?
                    .flatten(),
                ret_pointer_pointee: function
                    .ret_type
                    .as_ref()
                    .map(|ty| {
                        pointer_pointee_from_type(
                            ty,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )
                    })
                    .transpose()?
                    .flatten(),
            },
        );
        state.lowered.push((wrapper_name.clone(), wrapper_fn));
        Ok(wrapper_name)
    }

    fn resolve_closure(&mut self, name: &str, span: Span) -> Result<TypedValueIR, PinkerError> {
        if self
            .context
            .closure_state
            .borrow()
            .captures
            .contains_key(name)
        {
            return Err(PinkerError::Ir {
                msg: format!(
                    "closure '{}' referenciada mais de uma vez (não suportado nesta fase)",
                    name
                ),
                span,
            });
        }
        let function = self
            .context
            .all_functions
            .get(name)
            .cloned()
            .ok_or_else(|| PinkerError::Ir {
                msg: format!("lowering falhou ao resolver closure '{}'", name),
                span,
            })?;
        let param_names: HashSet<String> = function.params.iter().map(|p| p.name.clone()).collect();
        let free = transitive_free_identifiers_in_function(&function, |name| {
            self.context.all_functions.get(name).cloned()
        });
        let mut captures: Vec<CaptureMetadata> = Vec::new();
        let mut capture_values: Vec<ValueIR> = Vec::new();
        for candidate in &free {
            if param_names.contains(candidate) {
                continue;
            }
            let Some(binding) = self.resolve_existing_binding(candidate) else {
                continue;
            };
            captures.push(CaptureMetadata {
                source_name: candidate.clone(),
                ty: binding.ty,
                // A captura preserva a identidade exata da variável capturada:
                // um `ninho` capturado continua sendo aquele `ninho` dentro do
                // corpo da closure.
                resolved: binding.resolved,
                trait_object_name: self.trait_object_names.get(&binding.slot).cloned(),
                callable: self.callable_metadata.get(&binding.slot).cloned(),
                raw_function: self.raw_function_metadata.get(&binding.slot).cloned(),
                pointer_pointee: self.pointer_pointee_types.get(&binding.slot).copied(),
            });
            capture_values.push(ValueIR::Local(binding.slot));
        }
        self.context
            .closure_state
            .borrow_mut()
            .captures
            .insert(name.to_string(), captures.clone());
        let lowered =
            FunctionLowerer::new(self.context).lower_closure_function(&function, &captures)?;
        self.context
            .closure_state
            .borrow_mut()
            .lowered
            .push((name.to_string(), lowered));
        // A closure é um valor de função: sua identidade é a assinatura
        // declarada, do mesmo modo que um nome de função usado como valor.
        let closure_identity = self.function_value_identity(name, span)?;
        Ok(TypedValueIR {
            value: ValueIR::MakeClosure {
                function_name: name.to_string(),
                captures: capture_values,
            },
            ty: TypeIR::Function,
            resolved: closure_identity,
            ptr_array_bombom_size: None,
        })
    }

    // Fase 243: abaixa o corpo de uma closure com o ambiente já resolvido.
    // Quando há capturas, injeta um parâmetro oculto final `__env` (ponteiro)
    // e, antes do corpo real, uma sequência de `Let` sintéticos que
    // dereferenciam `__env + i*8` para cada captura (ordem determinística
    // de primeira referência) — mesma disciplina de 1 palavra por valor da
    // Fase 242. Sem chamada de usuário alcança este parâmetro: só a própria
    // chamada indireta o preenche (ver `cfg_ir`/`backend_s`).
    fn lower_closure_function(
        mut self,
        function: &FunctionDecl,
        captures: &[CaptureMetadata],
    ) -> Result<FunctionIR, PinkerError> {
        self.push_scope();

        // Fase 243: `__env` é SEMPRE o parâmetro real final (trailing) —
        // uniforme para toda função indiretamente chamável, capturante ou
        // não (closures sem captura E wrappers de função top-level, ver
        // `ensure_fnref_wrapper`). Isso é o que permite ao call site de
        // `call_indirect` emitir sempre N+1 argumentos sem ramificação:
        // quem não usa `__env` simplesmente o ignora. O slot é alocado
        // primeiro (para as expressões de desempacotamento abaixo), mas só
        // entra em `self.params` (posição final) depois dos parâmetros
        // reais.
        let env_pointer = TypeIR::Pointer { is_volatile: false };
        let env_identity = self
            .context
            .internal_identity("env", env_pointer, function.span)?;
        let env_binding =
            self.allocate_binding("__env", env_pointer, Some(env_identity), None, None)?;

        let mut prelude = Vec::new();
        for (index, capture) in captures.iter().enumerate() {
            // Capturas entram no escopo ANTES dos parâmetros para que um
            // parâmetro homônimo possa sombreá-las (§14.3) — a inserção
            // posterior do parâmetro no mesmo mapa de escopo sobrescreve.
            let capture_binding = self.allocate_binding(
                &capture.source_name,
                capture.ty,
                capture.resolved,
                None,
                Some(false),
            )?;
            if let Some(trait_name) = &capture.trait_object_name {
                self.trait_object_names
                    .insert(capture_binding.slot.clone(), trait_name.clone());
            }
            if let Some(callable) = &capture.callable {
                self.callable_metadata
                    .insert(capture_binding.slot.clone(), callable.clone());
            }
            if let Some(raw_function) = &capture.raw_function {
                self.raw_function_metadata
                    .insert(capture_binding.slot.clone(), raw_function.clone());
            }
            if let Some(pointer_pointee) = capture.pointer_pointee {
                self.pointer_pointee_types
                    .insert(capture_binding.slot.clone(), pointer_pointee);
            }
            let ptr_expr = ValueIR::Binary {
                op: BinaryOpIR::Add,
                lhs: Box::new(ValueIR::Local(env_binding.slot.clone())),
                rhs: Box::new(ValueIR::Int((index as u64) * 8)),
                ty: TypeIR::Pointer { is_volatile: false },
            };
            prelude.push(InstructionIR::Let {
                slot: capture_binding.slot,
                value: ValueIR::Deref {
                    ptr: Box::new(ptr_expr),
                    result_type: capture.ty,
                    is_volatile: false,
                },
                span: function.span,
            });
        }

        for param in &function.params {
            let binding = self.allocate_binding(
                &param.name,
                self.context.resolve_type(&param.ty)?,
                Some(self.context.resolved_identity(&param.ty)?),
                pointer_to_bombom_array_size(&param.ty, &self.context.type_aliases),
                None,
            )?;

            if let Some(trait_name) = trait_object_name_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.trait_object_names
                    .insert(binding.slot.clone(), trait_name);
            }

            if let Type::Function { ret, .. } = &param.ty {
                let ret_ty = self.context.resolve_type(ret)?;
                self.callable_metadata.insert(
                    binding.slot.clone(),
                    CallableMetadata {
                        ret_type: ret_ty,
                        ret_ast: Some(ret.as_ref().clone()),
                        ret_trait_name: trait_object_name_from_type(
                            ret,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )?,
                        ret_pointer_pointee: pointer_pointee_from_type(
                            ret,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )?,
                    },
                );
            }
            if let Some(metadata) = raw_function_metadata_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.raw_function_metadata
                    .insert(binding.slot.clone(), metadata);
            }
            if let Some(pointee) = pointer_pointee_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.pointer_pointee_types
                    .insert(binding.slot.clone(), pointee);
            }
            self.params.push(binding);
        }
        self.params.push(env_binding);

        let mut entry = self.lower_block(&function.body, "entry".to_string(), false)?;
        entry.instructions.splice(0..0, prelude);

        self.pop_scope();

        Ok(FunctionIR {
            name: function.name.clone(),
            params: self.params,
            locals: self.locals,
            ret_type: TypeIR::from_ast_option_with_context(
                function.ret_type.as_ref(),
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            entry,
            span: function.span,
        })
    }

    fn lower_block(
        &mut self,
        block: &Block,
        label: String,
        create_scope: bool,
    ) -> Result<BlockIR, PinkerError> {
        if create_scope {
            self.push_scope();
        }

        let mut instructions = Vec::new();
        for stmt in &block.stmts {
            instructions.push(self.lower_stmt(stmt)?);
        }

        if create_scope {
            self.pop_scope();
        }

        Ok(BlockIR {
            label,
            instructions,
            span: block.span,
        })
    }
    // @pinker-nav:end ir.lowering.funcoes-blocos

    // @pinker-nav:start ir.lowering.comandos-controle
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Abaixa comandos AST de um bloco para `InstructionIR`: despacho de `Stmt`, declaração local (`nova`/`muda`, incluindo o desvio de `lista_criar`/`mapa_criar` para o criar monomórfico anotado), atribuição a slot/deref/campo/índice, retorno (`mimo`), `falar`, asm inline, e o controle estruturado `talvez`/`senão` e `sempre que` com `quebrar`/`continuar` carregando destinos simbólicos de laço. Preserva spans; `if`/`while` continuam com blocos filhos — a divisão em blocos básicos ocorre depois em `cfg_ir`.
    fn lower_stmt(&mut self, stmt: &Stmt) -> Result<InstructionIR, PinkerError> {
        match stmt {
            Stmt::Let(let_stmt) => self.lower_let(let_stmt),
            Stmt::Assign(assign_stmt) => {
                let value = self.lower_value(&assign_stmt.expr)?;
                match &assign_stmt.target {
                    AssignTarget::Ident(name) => {
                        let binding = self.resolve_binding(name, assign_stmt.span)?;
                        if binding.ty == TypeIR::Function {
                            let metadata = self
                                .callable_metadata_for_expr(&assign_stmt.expr)?
                                .or_else(|| self.callable_metadata_for_value(&value.value))
                                .ok_or_else(|| {
                                    PinkerError::Ir {
                                        msg: format!(
                                            "lowering perdeu os metadados na reatribuição do callable '{}'",
                                            name
                                        ),
                                        span: assign_stmt.span,
                                    }
                                })?;
                            self.callable_metadata
                                .insert(binding.slot.clone(), metadata);
                        }
                        Ok(InstructionIR::Assign {
                            slot: binding.slot,
                            value: value.value,
                            span: assign_stmt.span,
                        })
                    }
                    AssignTarget::Deref(ptr_expr) => {
                        let pointee_type = self.pointer_pointee_for_expr(ptr_expr)?;
                        let ptr = self.lower_value(ptr_expr)?;
                        let is_volatile = match ptr.ty {
                            TypeIR::Pointer { is_volatile } => is_volatile,
                            _ => {
                                return Err(PinkerError::Ir {
                                    msg: "escrita indireta exige ponteiro no lowering IR"
                                        .to_string(),
                                    span: assign_stmt.span,
                                });
                            }
                        };
                        Ok(InstructionIR::StoreIndirect {
                            ptr: ptr.value,
                            value: value.value,
                            value_type: pointee_type.unwrap_or(value.ty),
                            is_volatile,
                            span: assign_stmt.span,
                        })
                    }
                    AssignTarget::Index { base, index } => {
                        let base_lowered = self.lower_value(base)?;
                        let element_type = match base_lowered.ty {
                            TypeIR::FixedArray { element, .. } => match element {
                                ScalarTypeIR::Bombom => TypeIR::Bombom,
                                _ => return Err(PinkerError::Ir {
                                    msg:
                                        "escrita por índice nesta fase aceita apenas '[bombom; N]'"
                                            .to_string(),
                                    span: assign_stmt.span,
                                }),
                            },
                            _ => {
                                return Err(PinkerError::Ir {
                                    msg:
                                        "escrita por índice exige base de array fixo no lowering IR"
                                            .to_string(),
                                    span: assign_stmt.span,
                                })
                            }
                        };
                        let index_lowered = self.lower_value(index)?;
                        Ok(InstructionIR::StoreIndexed {
                            base: base_lowered.value,
                            index: index_lowered.value,
                            value: value.value,
                            element_type,
                            span: assign_stmt.span,
                        })
                    }
                    AssignTarget::FieldDeref { base, field } => {
                        let base_lowered = self.lower_value(base)?;
                        let Some(base_struct_name) = self.nominal_name_of_value(&base_lowered)
                        else {
                            return Err(PinkerError::Ir {
                                msg: "escrita a campo exige base do tipo 'ninho' no lowering IR"
                                    .to_string(),
                                span: assign_stmt.span,
                            });
                        };
                        let base_struct_name = &base_struct_name;
                        let field_type = self
                            .context
                            .struct_fields
                            .get(base_struct_name)
                            .and_then(|fields| fields.get(field.as_str()))
                            .copied()
                            .ok_or_else(|| PinkerError::Ir {
                                msg: format!(
                                    "campo '{}' não encontrado em '{}' para escrita",
                                    field, base_struct_name
                                ),
                                span: assign_stmt.span,
                            })?;
                        let field_offset = self
                            .context
                            .struct_field_offsets
                            .get(base_struct_name)
                            .and_then(|fields| fields.get(field.as_str()))
                            .copied()
                            .ok_or_else(|| PinkerError::Ir {
                                msg: format!(
                                    "offset de campo '{}' não encontrado no layout de '{}' para escrita",
                                    field, base_struct_name
                                ),
                                span: assign_stmt.span,
                            })?;
                        let is_volatile = match &base_lowered.value {
                            ValueIR::Deref { is_volatile, .. } => *is_volatile,
                            _ => false,
                        };
                        Ok(InstructionIR::StoreFieldIndirect {
                            base: base_lowered.value,
                            field: field.clone(),
                            field_offset,
                            value: value.value,
                            value_type: field_type,
                            is_volatile,
                            span: assign_stmt.span,
                        })
                    }
                }
            }
            Stmt::Return(return_stmt) => self.lower_return(return_stmt),
            Stmt::Expr(expr) => Ok(InstructionIR::Expr {
                value: self.lower_value(expr)?.value,
                span: expr.span,
            }),
            Stmt::If(if_stmt) => self.lower_if(if_stmt),
            Stmt::While(while_stmt) => self.lower_while(while_stmt),
            Stmt::Break(break_stmt) => self.lower_break(break_stmt),
            Stmt::Continue(continue_stmt) => self.lower_continue(continue_stmt),
            Stmt::Falar(falar_stmt) => self.lower_falar(falar_stmt),
            Stmt::InlineAsm(inline_asm_stmt) => self.lower_inline_asm(inline_asm_stmt),
            Stmt::EnumMatch(enum_match) => self.lower_enum_match(enum_match),
            Stmt::UnionMatch(union_match) => self.lower_union_match(union_match),
        }
    }

    fn lower_enum_match(
        &mut self,
        enum_match: &EnumMatchStmt,
    ) -> Result<InstructionIR, PinkerError> {
        let scrutinee = self.lower_value(&enum_match.scrutinee)?;
        let scrutinee_identity = scrutinee.identity(self.context, enum_match.scrutinee.span)?;

        self.push_scope();
        let scrutinee_binding = self.allocate_binding(
            "encaixe_leque_alvo",
            scrutinee.ty,
            Some(scrutinee_identity),
            None,
            Some(false),
        )?;
        self.pop_scope();

        let mut arms = Vec::with_capacity(enum_match.arms.len());
        for arm in &enum_match.arms {
            self.push_scope();
            let pattern = self.lower_enum_pattern(&arm.pattern, scrutinee_identity)?;
            let body_label = self.next_block_label("encaixe_leque_braco");
            let body = self.lower_block(&arm.body, body_label, false);
            self.pop_scope();
            arms.push(EnumMatchArmIR {
                pattern,
                body: body?,
                span: arm.span,
            });
        }
        let otherwise = enum_match
            .otherwise
            .as_ref()
            .map(|block| {
                let label = self.next_block_label("encaixe_leque_senao");
                self.lower_block(block, label, true)
            })
            .transpose()?;

        Ok(InstructionIR::EnumMatch(EnumMatchIR {
            scrutinee: scrutinee.value,
            scrutinee_binding,
            arms,
            otherwise,
            span: enum_match.span,
        }))
    }

    fn lower_enum_pattern(
        &mut self,
        pattern: &EnumPattern,
        expected_type_id: ResolvedTypeId,
    ) -> Result<EnumPatternIR, PinkerError> {
        match pattern {
            EnumPattern::Binding { name, span } => Err(PinkerError::Ir {
                msg: format!(
                    "binding raiz '{}' não é permitido em 'encaixe' de leque",
                    name
                ),
                span: *span,
            }),
            EnumPattern::Variant {
                enum_name,
                variant,
                payloads,
                span,
            } => {
                let enum_info = self
                    .context
                    .enum_variants
                    .get(enum_name)
                    .cloned()
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!("leque '{}' do padrão ausente na IR", enum_name),
                        span: *span,
                    })?;
                let enum_identity = self.context.resolved_identity(&Type::Enum {
                    name: enum_info.declared_name.clone(),
                    span: *span,
                })?;
                if enum_identity != expected_type_id {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "INVALID_NESTED_PATTERN_TYPE: identidade esperada {} difere do leque '{}' ({})",
                            expected_type_id.0, enum_info.declared_name, enum_identity.0
                        ),
                        span: *span,
                    });
                }
                let (discriminant, declared_payloads) = enum_info
                    .variants
                    .get(variant)
                    .cloned()
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!(
                            "variante '{}.{}' do padrão ausente na IR",
                            enum_info.declared_name, variant
                        ),
                        span: *span,
                    })?;
                if declared_payloads.len() != payloads.len() {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "INVALID_PATTERN_PAYLOAD_ARITY: '{}.{}' exige {}, padrão possui {}",
                            enum_info.declared_name,
                            variant,
                            declared_payloads.len(),
                            payloads.len()
                        ),
                        span: *span,
                    });
                }

                let mut lowered_payloads = Vec::with_capacity(payloads.len());
                for (index, (payload, declared)) in
                    payloads.iter().zip(declared_payloads).enumerate()
                {
                    let resolved_type_id = self
                        .context
                        .intern_resolved_ast(&declared.shape.resolved, payload.span())?;
                    let lowered_pattern = match payload {
                        EnumPattern::Binding { name, span } => EnumPatternIR::Binding {
                            binding: self.allocate_binding(
                                name,
                                declared.operational_type,
                                Some(resolved_type_id),
                                None,
                                Some(false),
                            )?,
                            span: *span,
                        },
                        EnumPattern::Variant { .. } => {
                            self.lower_enum_pattern(payload, resolved_type_id)?
                        }
                    };
                    lowered_payloads.push(EnumPatternPayloadIR {
                        index: index as u64,
                        operational_type: declared.operational_type,
                        class: declared.shape.class,
                        canonical_key: declared.shape.canonical_key(),
                        resolved_type_id,
                        extract_intrinsic: declared.shape.carga_intrinsic().to_string(),
                        extracted_binding: self.allocate_binding(
                            "encaixe_leque_carga",
                            declared.operational_type,
                            Some(resolved_type_id),
                            None,
                            Some(false),
                        )?,
                        pattern: Box::new(lowered_pattern),
                    });
                }

                Ok(EnumPatternIR::Variant {
                    enum_name: enum_info.declared_name,
                    expected_type_id: enum_identity,
                    variant_name: variant.clone(),
                    discriminant,
                    has_payload: enum_info.has_payload,
                    payloads: lowered_payloads,
                    span: *span,
                })
            }
        }
    }

    // Abaixa `Stmt::UnionMatch` para `InstructionIR::UnionMatch`: avalia o scrutinee uma única vez, obtém o `UnionTypeId` do valor, resolve cada tipo de braço pelo contrato compartilhado de `union_canon`, localiza exatamente um membro do `UnionTypeIR` internado pela chave canônica, **copia** tag, tipo, tamanho e alinhamento do membro, cria o binding próprio do braço e abaixa o corpo no escopo desse binding. Preserva a ordem de fonte dos braços e revalida a cobertura como defesa de fronteira; a tag nunca é derivada de posição, ordem textual, nome de apelido ou `TypeIR` isolado.
    fn lower_union_match(
        &mut self,
        union_match: &UnionMatchStmt,
    ) -> Result<InstructionIR, PinkerError> {
        let scrutinee = self.lower_value(&union_match.scrutinee)?;
        let TypeIR::Union(union_type_id) = scrutinee.ty else {
            return Err(PinkerError::Ir {
                msg: format!(
                    "'encaixe' de união exige scrutinee de união; encontrado '{}'",
                    scrutinee.ty.name()
                ),
                span: union_match.scrutinee.span,
            });
        };

        let union_ir = {
            let registry = self.context.union_registry.borrow();
            registry
                .types
                .get(union_type_id.0 as usize)
                .cloned()
                .ok_or_else(|| PinkerError::Ir {
                    msg: format!(
                        "união {} ausente do registro internado no 'encaixe'",
                        union_type_id.0
                    ),
                    span: union_match.span,
                })?
        };

        // Slots de lowering para o scrutinee e a tag. São slots normalizados
        // desta camada (como qualquer `%nome#N`), não identidade de membro:
        // a identidade continua sendo a chave canônica do registry.
        self.push_scope();
        let scrutinee_binding = self.allocate_binding(
            "encaixe_uniao_alvo",
            TypeIR::Union(union_type_id),
            None,
            None,
            Some(false),
        )?;
        let tag_binding =
            self.allocate_binding("encaixe_uniao_tag", TypeIR::Bombom, None, None, Some(false))?;
        self.pop_scope();

        let mut arms = Vec::with_capacity(union_match.arms.len());
        let mut covered = HashSet::<String>::new();
        for arm in &union_match.arms {
            let resolved_member = self
                .context
                .resolve_union_ast_type(&arm.member_type, &mut Vec::new())?;
            let key = union_canon::member_key(&resolved_member);
            let mut matching = union_ir
                .members
                .iter()
                .filter(|member| member.canonical_member_key == key.canonical_type_key);
            let member = matching.next().ok_or_else(|| PinkerError::Ir {
                msg: format!(
                    "braço '{}' de 'encaixe' não pertence à união {}",
                    arm.member_type.name(),
                    union_type_id.0
                ),
                span: arm.span,
            })?;
            if matching.next().is_some() {
                return Err(PinkerError::Ir {
                    msg: format!(
                        "chave canônica ambígua na união {}: '{}'",
                        union_type_id.0, key.canonical_type_key
                    ),
                    span: arm.span,
                });
            }
            if !covered.insert(member.canonical_member_key.clone()) {
                return Err(PinkerError::Ir {
                    msg: format!(
                        "membro '{}' repetido no 'encaixe' da união {}",
                        member.canonical_member_key, union_type_id.0
                    ),
                    span: arm.span,
                });
            }

            self.push_scope();
            // O `encaixe` liga o braço à identidade **exata** do membro: o valor
            // desempacotado é aquele membro, não "algum membro com a mesma
            // representação". É isso que permite reinjetar o valor na mesma
            // união sem reescolher a tag.
            let binding = self.allocate_binding(
                &arm.binding,
                member.ty,
                Some(member.resolved_type_id),
                None,
                Some(false),
            )?;
            let body_label = self.next_block_label("encaixe_uniao_braco");
            let body = self.lower_block(&arm.body, body_label, false);
            self.pop_scope();
            let body = body?;

            arms.push(UnionMatchArmIR {
                tag: member.tag,
                canonical_member_key: member.canonical_member_key.clone(),
                resolved_member_type_id: member.resolved_type_id,
                binding,
                payload_type: member.ty,
                payload_layout: member.payload_layout,
                body,
                span: arm.span,
            });
        }

        // Defesa de fronteira: a semântica já exigiu cobertura exata, e o
        // lowering recusa qualquer divergência restante.
        if covered.len() != union_ir.members.len() {
            return Err(PinkerError::Ir {
                msg: format!(
                    "cobertura incompleta no 'encaixe' da união {}: {} de {} membros",
                    union_type_id.0,
                    covered.len(),
                    union_ir.members.len()
                ),
                span: union_match.span,
            });
        }

        Ok(InstructionIR::UnionMatch(UnionMatchIR {
            scrutinee: scrutinee.value,
            scrutinee_binding,
            tag_binding,
            union_type_id,
            arms,
            span: union_match.span,
        }))
    }

    fn lower_falar(&mut self, falar_stmt: &FalarStmt) -> Result<InstructionIR, PinkerError> {
        let mut args = Vec::with_capacity(falar_stmt.args.len());
        for arg in &falar_stmt.args {
            let typed = self.lower_value(arg)?;
            args.push(FalarArgIR {
                value: typed.value,
                ty: typed.ty,
            });
        }
        Ok(InstructionIR::Falar {
            args,
            span: falar_stmt.span,
        })
    }

    fn lower_inline_asm(
        &mut self,
        inline_asm_stmt: &InlineAsmStmt,
    ) -> Result<InstructionIR, PinkerError> {
        let mut operands = Vec::new();
        for operand in &inline_asm_stmt.operands {
            let constraint =
                crate::inline_asm::parse_constraint(&operand.constraint).map_err(|error| {
                    PinkerError::Ir {
                        msg: error.to_string(),
                        span: operand.span,
                    }
                })?;
            match &operand.direction {
                crate::ast::InlineAsmDirection::Input => {
                    let value = self.lower_value(&operand.value)?;
                    operands.push(InlineAsmOperandIR::Input {
                        name: operand.name.clone(),
                        constraint,
                        value: value.value,
                        ty: value.ty,
                    });
                }
                crate::ast::InlineAsmDirection::Output => {
                    let ExprKind::Ident(target) = &operand.value.kind else {
                        return Err(PinkerError::Ir {
                            msg: "saida de sussurro perdeu alvo simples validado".to_string(),
                            span: operand.value.span,
                        });
                    };
                    let binding = self.resolve_binding(target, operand.value.span)?;
                    operands.push(InlineAsmOperandIR::Output {
                        name: operand.name.clone(),
                        constraint,
                        slot: binding.slot,
                        ty: binding.ty,
                    });
                }
                crate::ast::InlineAsmDirection::Unknown(direction) => {
                    return Err(PinkerError::Ir {
                        msg: format!("direção de operando de sussurro não validada: '{direction}'"),
                        span: operand.span,
                    });
                }
            }
        }
        let clobbers = inline_asm_stmt
            .clobbers
            .iter()
            .map(|clobber| {
                crate::inline_asm::parse_clobber(&clobber.name).map_err(|error| PinkerError::Ir {
                    msg: error.to_string(),
                    span: clobber.span,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(InstructionIR::InlineAsm {
            chunks: inline_asm_stmt.chunks.clone(),
            operands,
            clobbers,
            span: inline_asm_stmt.span,
        })
    }

    fn lower_let(&mut self, let_stmt: &LetStmt) -> Result<InstructionIR, PinkerError> {
        // `nova l: lista<...> = lista_criar();` — a criação genérica abaixa
        // para o criar monomorphizado do tipo anotado (semântica já validou).
        if let Some(annotated_ty) = let_stmt.ty.as_ref() {
            if is_generic_list_create_expr(&let_stmt.init) {
                let slot_ty = self.context.resolve_type(annotated_ty)?;
                let callee = match slot_ty {
                    TypeIR::ListVerso => "lista_verso_criar",
                    _ => "lista_bombom_criar",
                };
                let binding = self.allocate_binding(
                    &let_stmt.name,
                    slot_ty,
                    Some(self.context.resolved_identity(annotated_ty)?),
                    None,
                    Some(let_stmt.is_mut),
                )?;
                return Ok(InstructionIR::Let {
                    slot: binding.slot,
                    value: ValueIR::Call {
                        identidade: crate::intrinsics::identity::callee_identity_da_grafia_canonica(
                            callee,
                        ),
                        callee: callee.to_string(),
                        args: Vec::new(),
                        ret_type: slot_ty,
                    },
                    span: let_stmt.span,
                });
            }
            if is_generic_map_create_expr(&let_stmt.init) {
                let slot_ty = self.context.resolve_type(annotated_ty)?;
                // U-02: a classe monomórfica não escolhe grafia aqui. O
                // adapter diz QUAL classe é, e a autoridade de especialização
                // diz qual identidade a operação `criar` endereça nela. O
                // fallback adulto do mapa genérico é outra pergunta, com outro
                // dono: ele não tem identidade monomórfica pública e continua
                // sendo materializado por operação interna.
                let callee = match canonical_map_class(slot_ty) {
                    Some(class) => crate::map_specialization::specialize_spelling(
                        class,
                        crate::map_specialization::GenericMapOperation::Criar,
                    ),
                    None => match slot_ty {
                        TypeIR::Map {
                            key: MapKeyIR::Bombom,
                            ..
                        } => "__pinker_internal_mapa_criar_chave_bombom",
                        TypeIR::Map {
                            key: MapKeyIR::Verso,
                            ..
                        } => "__pinker_internal_mapa_criar_chave_verso",
                        _ => {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "mapa_criar() exige anotação de mapa; encontrado '{}'",
                                    slot_ty.name()
                                ),
                                span: let_stmt.span,
                            });
                        }
                    },
                };
                let binding = self.allocate_binding(
                    &let_stmt.name,
                    slot_ty,
                    Some(self.context.resolved_identity(annotated_ty)?),
                    None,
                    Some(let_stmt.is_mut),
                )?;
                return Ok(InstructionIR::Let {
                    slot: binding.slot,
                    value: ValueIR::Call {
                        identidade: crate::intrinsics::identity::callee_identity_da_grafia_canonica(
                            callee,
                        ),
                        callee: callee.to_string(),
                        args: Vec::new(),
                        ret_type: slot_ty,
                    },
                    span: let_stmt.span,
                });
            }
        }
        let trait_object_name = match let_stmt.ty.as_ref() {
            Some(ty) => trait_object_name_from_type(
                ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            None => None,
        }
        .or(self.trait_object_name_for_expr(&let_stmt.init)?);

        let value = self.lower_value(&let_stmt.init)?;
        let ty = if let Some(annotated_ty) = let_stmt.ty.as_ref() {
            self.context.resolve_type(annotated_ty)?
        } else {
            value.ty
        };
        // A identidade do slot vem da anotação quando ela existe (é ela que o
        // usuário escreveu) e, na ausência dela, da identidade exata do valor.
        // Nenhuma das duas é derivada de nome textual.
        let resolved = match let_stmt.ty.as_ref() {
            Some(annotated_ty) => Some(self.context.resolved_identity(annotated_ty)?),
            None => value.resolved,
        };
        let ptr_array_bombom_size = let_stmt
            .ty
            .as_ref()
            .and_then(|annotated_ty| {
                pointer_to_bombom_array_size(annotated_ty, &self.context.type_aliases)
            })
            .or(value.ptr_array_bombom_size);
        // Fase 242: quando a variável é callable, registra o ret_type da
        // chamada indireta através dela — via anotação explícita, ou
        // derivado da origem do valor (referência de função, cópia de outra
        // variável callable, ou retorno de uma função que devolve callable).
        let callable_metadata = if ty == TypeIR::Function {
            if let Some(Type::Function { ret, .. }) = let_stmt.ty.as_ref() {
                Some(self.callable_metadata_from_return_type(ret)?)
            } else {
                self.callable_metadata_for_value(&value.value)
            }
        } else {
            None
        };
        let raw_function_metadata = if ty == TypeIR::FunctionPointer {
            if let Some(annotated_ty) = let_stmt.ty.as_ref() {
                raw_function_metadata_from_type(
                    annotated_ty,
                    &self.context.type_aliases,
                    &self.context.struct_names,
                )?
            } else {
                self.raw_function_metadata_for_value(&value.value)?
            }
        } else {
            None
        };
        let pointer_pointee_type = if let Some(annotated_ty) = let_stmt.ty.as_ref() {
            pointer_pointee_from_type(
                annotated_ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?
        } else {
            self.pointer_pointee_for_expr(&let_stmt.init)?
        };
        let binding = self.allocate_binding(
            &let_stmt.name,
            ty,
            resolved,
            ptr_array_bombom_size,
            Some(let_stmt.is_mut),
        )?;
        if let Some(metadata) = callable_metadata {
            self.callable_metadata
                .insert(binding.slot.clone(), metadata);
        }
        if let Some(metadata) = raw_function_metadata {
            self.raw_function_metadata
                .insert(binding.slot.clone(), metadata);
        }
        if let Some(pointee) = pointer_pointee_type {
            self.pointer_pointee_types
                .insert(binding.slot.clone(), pointee);
        }

        if ty == TypeIR::TraitObject {
            let trait_name = trait_object_name.ok_or_else(|| PinkerError::Ir {
                msg: format!(
                    "lowering perdeu a identidade nominal do objeto de trato '{}'",
                    let_stmt.name
                ),
                span: let_stmt.span,
            })?;

            self.trait_object_names
                .insert(binding.slot.clone(), trait_name);
        }

        Ok(InstructionIR::Let {
            slot: binding.slot,
            value: value.value,
            span: let_stmt.span,
        })
    }

    fn lower_return(&mut self, return_stmt: &ReturnStmt) -> Result<InstructionIR, PinkerError> {
        let value = return_stmt
            .expr
            .as_ref()
            .map(|expr| self.lower_value(expr).map(|typed| typed.value))
            .transpose()?;
        Ok(InstructionIR::Return {
            value,
            span: return_stmt.span,
        })
    }

    fn lower_if(&mut self, if_stmt: &IfStmt) -> Result<InstructionIR, PinkerError> {
        let condition = self.lower_value(&if_stmt.condition)?.value;
        let then_label = self.next_block_label("then");
        let then_block = self.lower_block(&if_stmt.then_branch, then_label, true)?;
        let else_block = match &if_stmt.else_branch {
            Some(ElseBlock::Block(block)) => {
                let else_label = self.next_block_label("else");
                Some(self.lower_block(block, else_label, true)?)
            }
            Some(ElseBlock::If(nested_if)) => {
                let else_label = self.next_block_label("else");
                self.push_scope();
                let nested_instruction = self.lower_if(nested_if)?;
                self.pop_scope();
                Some(BlockIR {
                    label: else_label,
                    instructions: vec![nested_instruction],
                    span: nested_if.span,
                })
            }
            None => None,
        };

        Ok(InstructionIR::If {
            condition,
            then_block,
            else_block,
            span: if_stmt.span,
        })
    }

    fn lower_while(&mut self, while_stmt: &WhileStmt) -> Result<InstructionIR, PinkerError> {
        let condition = self.lower_value(&while_stmt.condition)?.value;
        let body_label = self.next_block_label("loop");
        let loop_exit_label = self.next_block_label("loop_break_join");
        let loop_continue_label = self.next_block_label("loop_continue");
        self.loop_exit_stack.push(loop_exit_label);
        self.loop_continue_stack.push(loop_continue_label);
        let body_block = self.lower_block(&while_stmt.body, body_label, true)?;
        self.loop_continue_stack.pop();
        self.loop_exit_stack.pop();
        Ok(InstructionIR::While {
            condition,
            body_block,
            span: while_stmt.span,
        })
    }

    fn lower_continue(
        &mut self,
        continue_stmt: &ContinueStmt,
    ) -> Result<InstructionIR, PinkerError> {
        let Some(loop_continue_label) = self.loop_continue_stack.last() else {
            return Err(PinkerError::Ir {
                msg: "lowering encontrou 'continuar' fora de loop".to_string(),
                span: continue_stmt.span,
            });
        };

        Ok(InstructionIR::Continue {
            loop_continue_label: loop_continue_label.clone(),
            span: continue_stmt.span,
        })
    }

    fn lower_break(&mut self, break_stmt: &BreakStmt) -> Result<InstructionIR, PinkerError> {
        let Some(loop_exit_label) = self.loop_exit_stack.last() else {
            return Err(PinkerError::Ir {
                msg: "lowering encontrou 'quebrar' fora de loop".to_string(),
                span: break_stmt.span,
            });
        };

        Ok(InstructionIR::Break {
            loop_exit_label: loop_exit_label.clone(),
            span: break_stmt.span,
        })
    }
    // @pinker-nav:end ir.lowering.comandos-controle

    // @pinker-nav:start ir.lowering.expressoes-valores
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Grande despachante que abaixa expressões AST para `TypedValueIR` (valor, representação e identidade resolvida): literais, bindings/globais, operadores, dereferência, chamadas, métodos, intrínsecas genéricas, construção/leitura de leque, campos, índices, cast, `peso` e `alinhamento`. Operações de lista/mapa que devolvem elemento preservam a identidade exata do container, inclusive leques representados como `bombom`; não executa nem seleciona instruções de máquina.
    fn lower_value(&mut self, expr: &Expr) -> Result<TypedValueIR, PinkerError> {
        match &expr.kind {
            ExprKind::IntLit(value) => Ok(TypedValueIR {
                value: ValueIR::Int(*value),
                ty: TypeIR::Bombom,
                resolved: None,
                ptr_array_bombom_size: None,
            }),
            ExprKind::BoolLit(value) => Ok(TypedValueIR {
                value: ValueIR::Bool(*value),
                ty: TypeIR::Logica,
                resolved: None,
                ptr_array_bombom_size: None,
            }),
            ExprKind::StringLit(value) => Ok(TypedValueIR {
                value: ValueIR::String(value.clone()),
                ty: TypeIR::Verso,
                resolved: None,
                ptr_array_bombom_size: None,
            }),
            // #532: intrínseca fora de posição de chamada não tem valor. A
            // recusa é da semântica, que a produz com o span e a grafia do
            // usuário; chegar aqui significa que a checagem foi pulada.
            ExprKind::Intrinsic(identity) => Err(PinkerError::Ir {
                msg: format!(
                    "lowering recebeu a intrínseca '{}' fora de posição de chamada",
                    identity.canonical_public_spelling()
                ),
                span: expr.span,
            }),
            ExprKind::InternalMapIterCreate(map) => {
                let map = self.lower_value(map)?;
                Ok(TypedValueIR {
                    value: ValueIR::Call {
                        identidade: crate::intrinsics::identity::CalleeIdentity::CompilerInternal,
                        callee: "__pinker_internal_mapa_verso_bombom_iterador_criar".to_string(),
                        args: vec![map.value],
                        ret_type: TypeIR::Bombom,
                    },
                    ty: TypeIR::Bombom,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::InternalMapIterNextKey(iterator) => {
                let iterator = self.lower_value(iterator)?;
                Ok(TypedValueIR {
                    value: ValueIR::Call {
                        identidade: crate::intrinsics::identity::CalleeIdentity::CompilerInternal,
                        callee: "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave"
                            .to_string(),
                        args: vec![iterator.value],
                        ret_type: TypeIR::Verso,
                    },
                    ty: TypeIR::Verso,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Ident(name) => {
                // Fase 243: nome sintético de literal `carinho` — resolve
                // como criação de closure (com ou sem capturas), no ponto
                // exato onde `self.scopes` reflete o escopo léxico vigente.
                if name.starts_with("__anon_carinho_") {
                    return self.resolve_closure(name, expr.span);
                }
                if let Some(binding) = self.resolve_existing_binding(name) {
                    return Ok(TypedValueIR {
                        value: ValueIR::Local(binding.slot),
                        ty: binding.ty,
                        resolved: binding.resolved,
                        ptr_array_bombom_size: binding.ptr_array_bombom_size,
                    });
                }

                if let Some(ty) = self.context.global_consts.get(name) {
                    return Ok(TypedValueIR {
                        value: ValueIR::GlobalConst(name.clone()),
                        ty: *ty,
                        resolved: None,
                        ptr_array_bombom_size: None,
                    });
                }

                // Fase 242/243: nome solto de função top-level materializa
                // um valor callable. Desde a Fase 243, `FunctionRef` aponta
                // para um wrapper sintético (`__fnref_env_<nome>`) que
                // aceita e ignora o parâmetro oculto `__env` — a mesma
                // convenção uniforme das closures — sem alterar em nada a
                // função real nem suas chamadas diretas existentes.
                if self.context.function_sigs.contains_key(name) {
                    let wrapper_name = self.ensure_fnref_wrapper(name, expr.span)?;
                    // A identidade do valor callable é a assinatura completa:
                    // `carinho(u8) -> u8` e `carinho(u64) -> u64` compartilham
                    // `TypeIR::Function` e precisam de identidades distintas.
                    let resolved = self.function_value_identity(name, expr.span)?;
                    return Ok(TypedValueIR {
                        value: ValueIR::FunctionRef(wrapper_name),
                        ty: TypeIR::Function,
                        resolved,
                        ptr_array_bombom_size: None,
                    });
                }

                Err(PinkerError::Ir {
                    msg: format!("lowering falhou ao resolver identificador '{}'", name),
                    span: expr.span,
                })
            }
            ExprKind::AddressOf(operand) => {
                let ExprKind::Ident(name) = &operand.kind else {
                    return Err(PinkerError::Ir {
                        msg: "lowering de endereço cru exige função top-level resolvida"
                            .to_string(),
                        span: operand.span,
                    });
                };
                if !self.context.function_sigs.contains_key(name) {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "lowering não encontrou símbolo '{}' para endereço cru",
                            name
                        ),
                        span: operand.span,
                    });
                }
                // O endereço cru de uma função é `seta<carinho(...)>`: a
                // identidade é o ponteiro para a assinatura declarada, e não a
                // categoria `seta<carinho>`, que é a mesma para toda função.
                let signature = self.function_value_identity(name, expr.span)?;
                let resolved = match signature {
                    Some(signature) => {
                        let key = {
                            let table = self.context.resolved_types.borrow();
                            table.key_of(signature).map(str::to_string)
                        };
                        match key {
                            Some(key) => Some(
                                self.context
                                    .resolved_types
                                    .borrow_mut()
                                    .intern(
                                        format!("ptr:0:{key}"),
                                        TypeIR::FunctionPointer,
                                        ResolvedTypeParts {
                                            pointee: Some(signature),
                                            ..ResolvedTypeParts::default()
                                        },
                                    )
                                    .map_err(|msg| PinkerError::Ir {
                                        msg,
                                        span: expr.span,
                                    })?,
                            ),
                            None => None,
                        }
                    }
                    None => None,
                };
                Ok(TypedValueIR {
                    value: ValueIR::RawFunctionRef(name.clone()),
                    ty: TypeIR::FunctionPointer,
                    resolved,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Unary(op, operand) => {
                let pointee_type = if *op == UnaryOp::Deref {
                    self.pointer_pointee_for_expr(operand)?
                } else {
                    None
                };
                let operand = self.lower_value(operand)?;
                if *op == UnaryOp::Deref {
                    let TypeIR::Pointer { is_volatile } = operand.ty else {
                        return Err(PinkerError::Ir {
                            msg: "dereferência exige operando do tipo seta no lowering IR"
                                .to_string(),
                            span: expr.span,
                        });
                    };
                    // A identidade do valor dereferenciado é a identidade do
                    // apontado registrada na própria identidade do ponteiro:
                    // `seta<u8>` e `seta<u64>` compartilham `TypeIR::Pointer` e
                    // se distinguem exatamente aqui.
                    let pointee_identity = self.pointee_identity_of(&operand);
                    let (result_type, result_resolved) = match pointee_identity {
                        Some((pointee_id, TypeIR::Struct)) => (TypeIR::Struct, Some(pointee_id)),
                        _ => {
                            if let Some(size) = operand.ptr_array_bombom_size {
                                (
                                    TypeIR::FixedArray {
                                        element: ScalarTypeIR::Bombom,
                                        size,
                                    },
                                    None,
                                )
                            } else if let Some(pointee_type) = pointee_type {
                                let resolved = pointee_identity
                                    .filter(|(_, repr)| *repr == pointee_type)
                                    .map(|(id, _)| id);
                                (pointee_type, resolved)
                            } else {
                                (TypeIR::Bombom, None)
                            }
                        }
                    };
                    return Ok(TypedValueIR {
                        value: ValueIR::Deref {
                            ptr: Box::new(operand.value),
                            result_type,
                            is_volatile,
                        },
                        ty: result_type,
                        resolved: result_resolved,
                        ptr_array_bombom_size: None,
                    });
                }
                Ok(TypedValueIR {
                    value: ValueIR::Unary {
                        op: UnaryOpIR::from_ast(*op),
                        operand: Box::new(operand.value),
                        ty: match op {
                            UnaryOp::Neg | UnaryOp::BitNot => operand.ty,
                            UnaryOp::Not => TypeIR::Logica,
                            UnaryOp::Deref => unreachable!("deref tratada acima"),
                        },
                    },
                    ty: match op {
                        UnaryOp::Neg => operand.ty,
                        UnaryOp::Not => TypeIR::Logica,
                        UnaryOp::BitNot => operand.ty,
                        UnaryOp::Deref => unreachable!("deref tratada acima"),
                    },
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let lhs_is_int_lit = matches!(lhs.kind, ExprKind::IntLit(_));
                let lhs = self.lower_value(lhs)?;
                let rhs = self.lower_value(rhs)?;
                if *op == BinaryOp::Add && matches!(lhs.ty, TypeIR::Pointer { .. }) {
                    let element_layout = self.pointer_element_layout(&lhs, expr.span)?;
                    let result_type = lhs.ty;
                    let result_resolved = lhs.resolved;
                    let result_array_size = lhs.ptr_array_bombom_size;
                    return Ok(TypedValueIR {
                        value: ValueIR::PointerOffset {
                            pointer: Box::new(lhs.value),
                            offset: Box::new(rhs.value),
                            pointer_type: result_type,
                            element_size: element_layout.size,
                            element_align: element_layout.align,
                        },
                        ty: result_type,
                        resolved: result_resolved,
                        ptr_array_bombom_size: result_array_size,
                    });
                }
                let operation_type = if lhs_is_int_lit && rhs.ty.is_integer() {
                    rhs.ty
                } else {
                    lhs.ty
                };
                let result_type = match op {
                    BinaryOp::LogicalAnd
                    | BinaryOp::LogicalOr
                    | BinaryOp::Eq
                    | BinaryOp::Neq
                    | BinaryOp::Lt
                    | BinaryOp::Lte
                    | BinaryOp::Gt
                    | BinaryOp::Gte => TypeIR::Logica,
                    _ => operation_type,
                };
                Ok(TypedValueIR {
                    value: ValueIR::Binary {
                        op: BinaryOpIR::from_ast(*op),
                        lhs: Box::new(lhs.value),
                        rhs: Box::new(rhs.value),
                        ty: operation_type,
                    },
                    ty: result_type,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Call(callee, args) => {
                // Construção `Leque.Variante(cargas...)` abaixa para uma
                // cadeia composável: criar_0(tag) seguido de um anexar por
                // carga, cada anexar devolvendo o mesmo handle.
                if let ExprKind::FieldAccess { base, field } = &callee.kind {
                    if let ExprKind::Ident(base_name) = &base.kind {
                        if let Some(info) = self.context.enum_variants.get(base_name) {
                            let Some((discriminant, payload_types)) =
                                info.variants.get(field).cloned()
                            else {
                                return Err(PinkerError::Ir {
                                    msg: format!(
                                        "construção inválida de '{}.{}' na IR",
                                        base_name, field
                                    ),
                                    span: expr.span,
                                });
                            };
                            if payload_types.is_empty() || args.len() != payload_types.len() {
                                return Err(PinkerError::Ir {
                                    msg: format!(
                                        "construção de '{}.{}' com aridade inconsistente na IR",
                                        base_name, field
                                    ),
                                    span: expr.span,
                                });
                            }
                            let mut chain = ValueIR::Call {
                                identidade:
                                    crate::intrinsics::identity::CalleeIdentity::CompilerInternal,
                                callee: "__pinker_internal_leque_criar_0".to_string(),
                                args: vec![ValueIR::Int(discriminant)],
                                ret_type: TypeIR::Bombom,
                            };
                            for (arg, payload_ty) in args.iter().zip(payload_types) {
                                let payload = self.lower_value(arg)?;
                                // A identidade da carga é conferida aqui, e não
                                // pela representação: `lista<Cor>` e
                                // `lista<Token>` são a mesma palavra e tipos
                                // diferentes.
                                let expected_identity = self
                                    .context
                                    .intern_resolved_ast(&payload_ty.shape.resolved, arg.span)?;
                                if let Some(actual) = payload.resolved {
                                    if actual != expected_identity {
                                        return Err(PinkerError::Ir {
                                            msg: format!(
                                                "E-IR-ENUM-PAYLOAD-IDENTITY: carga de '{}.{}' exige identidade '{}' e recebeu '{}'",
                                                base_name,
                                                field,
                                                payload_ty.shape.canonical_key(),
                                                self.context
                                                    .resolved_types
                                                    .borrow()
                                                    .key_of(actual)
                                                    .unwrap_or("?")
                                            ),
                                            span: arg.span,
                                        });
                                    }
                                }
                                // O helper deriva da classe de representação
                                // decidida pela autoridade única, nunca de um
                                // `match` parcial sobre o tipo-fonte.
                                let anexar = payload_ty.shape.anexar_intrinsic();
                                chain = ValueIR::Call {
                                    identidade:
                                        crate::intrinsics::identity::callee_identity_da_grafia_canonica(
                                            anexar,
                                        ),
                                    callee: anexar.to_string(),
                                    args: vec![chain, payload.value],
                                    ret_type: TypeIR::Bombom,
                                };
                            }
                            return Ok(TypedValueIR {
                                value: chain,
                                ty: TypeIR::Bombom,
                                resolved: None,
                                ptr_array_bombom_size: None,
                            });
                        }
                    }
                }
                if let ExprKind::FieldAccess { base, field } = &callee.kind {
                    if let ExprKind::Ident(trait_name) = &base.kind {
                        if self.context.traits.contains_key(trait_name) && !args.is_empty() {
                            let receiver = self.lower_value(&args[0])?;

                            if receiver.ty == TypeIR::TraitObject {
                                return self.lower_trait_call(
                                    receiver,
                                    trait_name,
                                    field,
                                    &args[1..],
                                    expr.span,
                                );
                            }

                            if let Some(function_name) = self.resolve_qualified_impl_method(
                                &receiver, trait_name, field, expr.span,
                            )? {
                                let mut ir_args = Vec::with_capacity(args.len());
                                ir_args.push(receiver.value);
                                for arg in args.iter().skip(1) {
                                    ir_args.push(self.lower_value(arg)?.value);
                                }
                                let ret_type = self
                                    .context
                                    .function_sigs
                                    .get(&function_name)
                                    .map(|sig| sig.ret_type)
                                    .ok_or_else(|| PinkerError::Ir {
                                        msg: format!(
                                            "lowering falhou ao resolver método interno '{}'",
                                            function_name
                                        ),
                                        span: expr.span,
                                    })?;
                                return Ok(TypedValueIR {
                                    value: ValueIR::Call {
                                        identidade:
                                            crate::intrinsics::identity::CalleeIdentity::User,
                                        callee: function_name.clone(),
                                        args: ir_args,
                                        ret_type,
                                    },
                                    ty: ret_type,
                                    resolved: self
                                        .context
                                        .function_sigs
                                        .get(&function_name)
                                        .map(|sig| sig.ret_resolved),
                                    ptr_array_bombom_size: None,
                                });
                            }
                        }
                    }
                    let receiver = self.lower_value(base)?;

                    if receiver.ty == TypeIR::TraitObject {
                        let trait_name =
                            self.trait_object_name_for_expr(base)?.ok_or_else(|| {
                                PinkerError::Ir {
                                    msg: format!(
                                        "lowering perdeu a identidade nominal do receiver de '{}'",
                                        field
                                    ),
                                    span: expr.span,
                                }
                            })?;

                        return self.lower_trait_call(
                            receiver,
                            &trait_name,
                            field,
                            args,
                            expr.span,
                        );
                    }

                    let function_name = if let Some(function_name) =
                        self.resolve_impl_method(&receiver, field, expr.span)?
                    {
                        function_name
                    } else if self.context.function_sigs.contains_key(field) {
                        field.clone()
                    } else {
                        return Err(PinkerError::Ir {
                            msg: format!(
                                "lowering falhou ao resolver método '{}' para receiver '{}'",
                                field,
                                self.impl_receiver_key(&receiver)
                                    .unwrap_or_else(|| receiver.ty.name().to_string())
                            ),
                            span: expr.span,
                        });
                    };
                    let mut ir_args = Vec::with_capacity(args.len() + 1);
                    ir_args.push(receiver.value);
                    for arg in args {
                        ir_args.push(self.lower_value(arg)?.value);
                    }
                    let ret_type = self
                        .context
                        .function_sigs
                        .get(&function_name)
                        .map(|sig| sig.ret_type)
                        .ok_or_else(|| PinkerError::Ir {
                            msg: format!(
                                "lowering falhou ao resolver método interno '{}'",
                                function_name
                            ),
                            span: expr.span,
                        })?;
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            identidade: crate::intrinsics::identity::CalleeIdentity::User,
                            callee: function_name.clone(),
                            args: ir_args,
                            ret_type,
                        },
                        ty: ret_type,
                        resolved: self
                            .context
                            .function_sigs
                            .get(&function_name)
                            .map(|sig| sig.ret_resolved),
                        ptr_array_bombom_size: None,
                    });
                }

                // #532: a mesma decisão que `semantic` consumiu chega aqui, e não
                // é reconstruída pelo texto. `name` continua sendo a grafia —
                // para escolher QUAL builtin —, mas quem diz SE a chamada é
                // builtin é `identidade_do_callee`.
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
                    ExprKind::Intrinsic(identity) => {
                        Some(identity.canonical_public_spelling().to_string())
                    }
                    ExprKind::Ident(name) => Some(name.clone()),
                    _ => None,
                };
                let Some(name) = grafia_do_callee.as_ref() else {
                    let lowered_callee = self.lower_value(callee)?;
                    if lowered_callee.ty != TypeIR::FunctionPointer {
                        return Err(PinkerError::Ir {
                            msg: "lowering de chamada por expressão exige ponteiro cru de função"
                                .to_string(),
                            span: callee.span,
                        });
                    }
                    let Some(metadata) =
                        self.raw_function_metadata_for_value(&lowered_callee.value)?
                    else {
                        return Err(PinkerError::Ir {
                            msg: "lowering perdeu a assinatura da expressão de ponteiro cru"
                                .to_string(),
                            span: callee.span,
                        });
                    };
                    let ir_args = args
                        .iter()
                        .map(|arg| self.lower_value(arg).map(|typed| typed.value))
                        .collect::<Result<Vec<_>, _>>()?;
                    let raw_resolved = self.raw_ret_identity(&metadata, expr.span)?;
                    return Ok(TypedValueIR {
                        value: ValueIR::CallRaw {
                            callee: Box::new(lowered_callee.value),
                            args: ir_args,
                            param_types: metadata.param_types,
                            ret_type: metadata.ret_type,
                        },
                        ty: metadata.ret_type,
                        resolved: raw_resolved,
                        ptr_array_bombom_size: None,
                    });
                };

                // Fase 242: variável local (parâmetro/`nova`) de tipo função
                // tem precedência sobre função top-level homônima — chamada
                // indireta real, callee é um valor (slot), não um símbolo.
                if let Some(binding) = self.resolve_existing_binding(name) {
                    if binding.ty == TypeIR::FunctionPointer {
                        let Some(metadata) = self.raw_function_metadata.get(&binding.slot).cloned()
                        else {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "lowering perdeu a assinatura do ponteiro cru de função '{}'",
                                    name
                                ),
                                span: expr.span,
                            });
                        };
                        let ir_args = args
                            .iter()
                            .map(|arg| self.lower_value(arg).map(|typed| typed.value))
                            .collect::<Result<Vec<_>, _>>()?;
                        let raw_resolved = self.raw_ret_identity(&metadata, expr.span)?;
                        return Ok(TypedValueIR {
                            value: ValueIR::CallRaw {
                                callee: Box::new(ValueIR::Local(binding.slot)),
                                args: ir_args,
                                param_types: metadata.param_types,
                                ret_type: metadata.ret_type,
                            },
                            ty: metadata.ret_type,
                            resolved: raw_resolved,
                            ptr_array_bombom_size: None,
                        });
                    }
                    if binding.ty == TypeIR::Function {
                        let Some(metadata) = self.callable_metadata.get(&binding.slot).cloned()
                        else {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "lowering falhou ao inferir retorno da chamada indireta de '{}' (encadeamento de callable retornando callable além de um nível não é suportado nesta fase)",
                                    name
                                ),
                                span: expr.span,
                            });
                        };
                        if metadata.ret_type == TypeIR::TraitObject
                            && metadata.ret_trait_name.is_none()
                        {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "lowering perdeu a identidade nominal do trato retornado pela chamada indireta de '{}'",
                                    name
                                ),
                                span: expr.span,
                            });
                        }
                        let typed_args: Vec<TypedValueIR> = args
                            .iter()
                            .map(|arg| self.lower_value(arg))
                            .collect::<Result<Vec<_>, _>>()?;
                        let ir_args: Vec<ValueIR> =
                            typed_args.into_iter().map(|typed| typed.value).collect();
                        let resolved = self.callable_ret_identity(&metadata, expr.span)?;
                        return Ok(TypedValueIR {
                            value: ValueIR::CallIndirect {
                                callee: Box::new(ValueIR::Local(binding.slot)),
                                args: ir_args,
                                ret_type: metadata.ret_type,
                            },
                            ty: metadata.ret_type,
                            resolved,
                            ptr_array_bombom_size: None,
                        });
                    }
                }

                // Intrínsecas genéricas de lista (Fase 211): abaixam para a
                // forma monomorphizada conforme o tipo da lista no argumento 1.
                //
                // #532: o lowering de builtin exige identidade de builtin. Uma
                // função do usuário com esta grafia não entra aqui.
                if identidade_do_callee.dispatches_as_builtin()
                    && matches!(
                        name.as_str(),
                        "lista_tamanho"
                            | "lista_obter"
                            | "lista_anexar"
                            | "lista_definir"
                            | "lista_tirar_ultimo"
                            | "lista_inserir"
                    )
                {
                    let typed_args: Vec<TypedValueIR> = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let prefix = match typed_args.first().map(|arg| arg.ty) {
                        Some(TypeIR::ListVerso) => "lista_verso",
                        _ => "lista_bombom",
                    };
                    let suffix = name.strip_prefix("lista").unwrap_or_default();
                    let mono_name = format!("{}{}", prefix, suffix);
                    let element_identity =
                        if matches!(name.as_str(), "lista_obter" | "lista_tirar_ultimo") {
                            typed_args.first().and_then(|list| {
                                list.resolved.and_then(|identity| {
                                    self.context
                                        .resolved_types
                                        .borrow()
                                        .get(identity)
                                        .and_then(|entry| entry.element)
                                })
                            })
                        } else {
                            None
                        };
                    let ret_type = self
                        .context
                        .function_sigs
                        .get(&mono_name)
                        .map(|sig| sig.ret_type)
                        .ok_or_else(|| PinkerError::Ir {
                            msg: format!(
                                "lowering falhou ao resolver intrínseca genérica '{}' ('{}')",
                                name, mono_name
                            ),
                            span: expr.span,
                        })?;
                    let ir_args: Vec<ValueIR> =
                        typed_args.into_iter().map(|typed| typed.value).collect();
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            identidade:
                                crate::intrinsics::identity::callee_identity_da_grafia_canonica(
                                    &mono_name,
                                ),
                            callee: mono_name,
                            args: ir_args,
                            ret_type,
                        },
                        ty: ret_type,
                        resolved: element_identity,
                        ptr_array_bombom_size: None,
                    });
                }

                if identidade_do_callee.dispatches_as_builtin()
                    && matches!(
                        name.as_str(),
                        "mapa_definir"
                            | "mapa_obter"
                            | "mapa_tem"
                            | "mapa_tamanho"
                            | "mapa_remover"
                    )
                {
                    let typed_args: Vec<TypedValueIR> = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let Some(first_arg) = typed_args.first() else {
                        return Err(PinkerError::Ir {
                            msg: format!(
                                "lowering falhou ao resolver intrínseca genérica '{}' sem mapa",
                                name
                            ),
                            span: expr.span,
                        });
                    };
                    if let Some(mono_name) = generic_map_monomorphic_callee(first_arg.ty, name) {
                        let ret_type = self
                            .context
                            .function_sigs
                            .get(mono_name)
                            .map(|sig| sig.ret_type)
                            .ok_or_else(|| PinkerError::Ir {
                                msg: format!(
                                    "lowering falhou ao resolver intrínseca genérica '{}' ('{}')",
                                    name, mono_name
                                ),
                                span: expr.span,
                            })?;
                        let ir_args: Vec<ValueIR> =
                            typed_args.into_iter().map(|typed| typed.value).collect();
                        return Ok(TypedValueIR {
                            value: ValueIR::Call {
                                identidade:
                                    crate::intrinsics::identity::callee_identity_da_grafia_canonica(
                                        mono_name,
                                    ),
                                callee: mono_name.to_string(),
                                args: ir_args,
                                ret_type,
                            },
                            ty: ret_type,
                            resolved: None,
                            ptr_array_bombom_size: None,
                        });
                    }
                    if let TypeIR::Map { value, .. } = first_arg.ty {
                        let value_identity = if name == "mapa_obter" {
                            let map_identity = first_arg.identity(self.context, expr.span)?;
                            let table = self.context.resolved_types.borrow();
                            let map_entry =
                                table.get(map_identity).ok_or_else(|| PinkerError::Ir {
                                    msg: format!(
                                        "identidade {} do mapa genérico ausente no lowering",
                                        map_identity.0
                                    ),
                                    span: expr.span,
                                })?;
                            Some(map_entry.element.ok_or_else(|| PinkerError::Ir {
                                msg: "identidade do valor de mapa genérico perdida antes de mapa_obter"
                                    .to_string(),
                                span: expr.span,
                            })?)
                        } else {
                            None
                        };
                        let ret_type = match name.as_str() {
                            "mapa_obter" => value.type_ir(),
                            "mapa_tem" => TypeIR::Logica,
                            "mapa_tamanho" => TypeIR::Bombom,
                            "mapa_definir" | "mapa_remover" => TypeIR::Nulo,
                            _ => unreachable!(),
                        };
                        let ir_args = typed_args.into_iter().map(|typed| typed.value).collect();
                        return Ok(TypedValueIR {
                            value: ValueIR::Call {
                                identidade:
                                    crate::intrinsics::identity::CalleeIdentity::CompilerInternal,
                                callee: format!("__pinker_internal_{name}"),
                                args: ir_args,
                                ret_type,
                            },
                            ty: ret_type,
                            resolved: value_identity,
                            ptr_array_bombom_size: None,
                        });
                    }
                }

                if identidade_do_callee.dispatches_as_builtin()
                    && matches!(
                        name.as_str(),
                        "__pinker_internal_mapa_iterador_criar"
                            | "__pinker_internal_mapa_iterador_proxima_chave_bombom"
                            | "__pinker_internal_mapa_iterador_proxima_chave_verso"
                    )
                {
                    let typed_args = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let ret_type = if name.ends_with("_verso") {
                        TypeIR::Verso
                    } else {
                        TypeIR::Bombom
                    };
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            identidade: identidade_do_callee,
                            callee: name.clone(),
                            args: typed_args.into_iter().map(|typed| typed.value).collect(),
                            ret_type,
                        },
                        ty: ret_type,
                        resolved: None,
                        ptr_array_bombom_size: None,
                    });
                }

                // `formatar_verso` (Fase 219/B8): argumentos `bombom` são
                // convertidos para verso já na IR (mesmo texto que o
                // interpretador produziria), permitindo que o runtime nativo
                // trate todos os argumentos uniformemente como versos.
                if identidade_do_callee.dispatches_as_builtin() && name == "formatar_verso" {
                    let mut ir_args = Vec::with_capacity(args.len());
                    for (idx, arg) in args.iter().enumerate() {
                        let typed = self.lower_value(arg)?;
                        if idx > 0 && typed.ty != TypeIR::Verso {
                            ir_args.push(ValueIR::Call {
                                identidade:
                                    crate::intrinsics::identity::callee_identity_da_grafia_canonica(
                                        "bombom_para_verso",
                                    ),
                                callee: "bombom_para_verso".to_string(),
                                args: vec![typed.value],
                                ret_type: TypeIR::Verso,
                            });
                        } else {
                            ir_args.push(typed.value);
                        }
                    }
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            identidade: identidade_do_callee,
                            callee: name.clone(),
                            args: ir_args,
                            ret_type: TypeIR::Verso,
                        },
                        ty: TypeIR::Verso,
                        resolved: None,
                        ptr_array_bombom_size: None,
                    });
                }

                if identidade_do_callee.dispatches_as_builtin() && name == "__ternario" {
                    let typed_args: Vec<TypedValueIR> = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let ret_type = typed_args[1].ty;
                    // Os dois ramos precisam concordar na identidade semântica,
                    // não apenas na representação: um ternário entre dois
                    // `ninho` diferentes (ou dois `leque` diferentes) não pode
                    // produzir um valor de identidade indeterminada.
                    // Ternário de callables tem contrato próprio (Fase 244): a
                    // concordância dos dois braços é exigida pela metadata de
                    // callable, com diagnósticos específicos. Aqui a identidade
                    // apenas acompanha o primeiro braço para não antecipar (e
                    // mascarar) aqueles diagnósticos.
                    if ret_type == TypeIR::Function {
                        let resolved = typed_args[1].resolved;
                        let ir_args: Vec<ValueIR> =
                            typed_args.into_iter().map(|t| t.value).collect();
                        return Ok(TypedValueIR {
                            value: ValueIR::Call {
                                identidade: identidade_do_callee,
                                callee: name.clone(),
                                args: ir_args,
                                ret_type,
                            },
                            ty: ret_type,
                            resolved,
                            ptr_array_bombom_size: None,
                        });
                    }
                    let resolved = match (typed_args[1].resolved, typed_args[2].resolved) {
                        (Some(left), Some(right)) => {
                            if left != right {
                                return Err(PinkerError::Ir {
                                    msg: format!(
                                        "E-IR-TYPE-IDENTITY-LOST: ramos do ternário têm \
                                         identidades resolvidas distintas ({} e {})",
                                        left.0, right.0
                                    ),
                                    span: expr.span,
                                });
                            }
                            Some(left)
                        }
                        (None, None) => None,
                        (Some(known), None) | (None, Some(known)) => {
                            // O ramo sem identidade explícita só é aceito quando
                            // sua representação já é a identidade completa e
                            // coincide com a do outro ramo.
                            let derived = self.context.repr_identity(ret_type, expr.span)?;
                            if derived != known {
                                return Err(PinkerError::Ir {
                                    msg: "E-IR-TYPE-IDENTITY-LOST: ramos do ternário não \
                                          concordam na identidade resolvida"
                                        .to_string(),
                                    span: expr.span,
                                });
                            }
                            Some(known)
                        }
                    };
                    let ir_args: Vec<ValueIR> = typed_args.into_iter().map(|t| t.value).collect();
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            identidade: identidade_do_callee,
                            callee: name.clone(),
                            args: ir_args,
                            ret_type,
                        },
                        ty: ret_type,
                        resolved,
                        ptr_array_bombom_size: None,
                    });
                }

                let args = args
                    .iter()
                    .map(|arg| self.lower_value(arg).map(|typed| typed.value))
                    .collect::<Result<Vec<_>, _>>()?;

                // #532: a assinatura consultada é a do callee que a
                // resolução escolheu. Um callee de usuário nunca é tipado pela
                // assinatura da intrínseca homônima.
                let sig = if identidade_do_callee.is_user() {
                    self.context.declared_sigs.get(name)
                } else {
                    self.context.function_sigs.get(name)
                };
                let ret_type = sig.map(|sig| sig.ret_type).ok_or_else(|| PinkerError::Ir {
                    msg: format!("lowering falhou ao resolver chamada '{}'", name),
                    span: expr.span,
                })?;
                let ret_resolved = sig.map(|sig| sig.ret_resolved);

                Ok(TypedValueIR {
                    value: ValueIR::Call {
                        identidade: identidade_do_callee,
                        callee: name.clone(),
                        args,
                        ret_type,
                    },
                    ty: ret_type,
                    resolved: ret_resolved,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::FieldAccess { base, field } => {
                // `Leque.Variante`: em leque sem carga vira o discriminante
                // imediato; em leque com carga vira um handle recém-criado.
                if let ExprKind::Ident(base_name) = &base.kind {
                    if let Some(info) = self.context.enum_variants.get(base_name) {
                        let Some((discriminant, _)) = info.variants.get(field) else {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "variante '{}' não existe no leque '{}'",
                                    field, base_name
                                ),
                                span: expr.span,
                            });
                        };
                        // O valor de uma variante **é** do leque de origem: a
                        // representação escalar não apaga a identidade nominal.
                        // Sem isto, dois `leque` distintos ficariam
                        // indistinguíveis na injeção em união (HR4).
                        let leque_identity = self.context.resolved_identity(&Type::Alias {
                            name: base_name.clone(),
                            span: base.span,
                        })?;
                        if info.has_payload {
                            return Ok(TypedValueIR {
                                value: ValueIR::Call {
                                    identidade:
                                        crate::intrinsics::identity::CalleeIdentity::CompilerInternal,
                                    callee: "__pinker_internal_leque_criar_0".to_string(),
                                    args: vec![ValueIR::Int(*discriminant)],
                                    ret_type: TypeIR::Bombom,
                                },
                                ty: TypeIR::Bombom,
                                resolved: Some(leque_identity),
                                ptr_array_bombom_size: None,
                            });
                        }
                        return Ok(TypedValueIR {
                            value: ValueIR::Int(*discriminant),
                            ty: TypeIR::Bombom,
                            resolved: Some(leque_identity),
                            ptr_array_bombom_size: None,
                        });
                    }
                }
                let base = self.lower_value(base)?;
                let Some(base_struct_name) = self.nominal_name_of_value(&base) else {
                    return Err(PinkerError::Ir {
                        msg: "acesso a campo com base não-struct na IR".to_string(),
                        span: expr.span,
                    });
                };
                let base_struct_name = &base_struct_name;
                let result_type = self
                    .context
                    .struct_fields
                    .get(base_struct_name)
                    .and_then(|fields| fields.get(field))
                    .copied()
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!("campo '{}' não encontrado em '{}'", field, base_struct_name),
                        span: expr.span,
                    })?;
                let field_offset = self
                    .context
                    .struct_field_offsets
                    .get(base_struct_name)
                    .and_then(|fields| fields.get(field))
                    .copied()
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!(
                            "offset de campo '{}' não encontrado no layout de '{}'",
                            field, base_struct_name
                        ),
                        span: expr.span,
                    })?;
                Ok(TypedValueIR {
                    value: ValueIR::FieldAccess {
                        base: Box::new(base.value),
                        field: field.clone(),
                        field_offset,
                        result_type,
                    },
                    ty: result_type,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Index { base, index } => {
                let base = self.lower_value(base)?;
                let index = self.lower_value(index)?;
                let TypeIR::FixedArray { element, .. } = base.ty else {
                    return Err(PinkerError::Ir {
                        msg: "indexação com base não-array na IR".to_string(),
                        span: expr.span,
                    });
                };
                let element_type = match element {
                    ScalarTypeIR::Bombom => TypeIR::Bombom,
                    ScalarTypeIR::U8 => TypeIR::U8,
                    ScalarTypeIR::U16 => TypeIR::U16,
                    ScalarTypeIR::U32 => TypeIR::U32,
                    ScalarTypeIR::U64 => TypeIR::U64,
                    ScalarTypeIR::I8 => TypeIR::I8,
                    ScalarTypeIR::I16 => TypeIR::I16,
                    ScalarTypeIR::I32 => TypeIR::I32,
                    ScalarTypeIR::I64 => TypeIR::I64,
                    ScalarTypeIR::Logica => TypeIR::Logica,
                };
                Ok(TypedValueIR {
                    value: ValueIR::Index {
                        base: Box::new(base.value),
                        index: Box::new(index.value),
                        element_type,
                    },
                    ty: element_type,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Cast {
                expr: source,
                target,
            } => {
                let lowered_source = self.lower_value(source)?;
                let target_type = self.context.resolve_type(target)?;

                if target_type == TypeIR::TraitObject {
                    let trait_name = trait_object_name_from_type(
                        target,
                        &self.context.type_aliases,
                        &self.context.struct_names,
                    )?
                    .ok_or_else(|| PinkerError::Ir {
                        msg: "materialização sem nome nominal de trato".to_string(),
                        span: expr.span,
                    })?;

                    let concrete_type_name =
                        self.impl_receiver_key(&lowered_source)
                            .ok_or_else(|| PinkerError::Ir {
                                msg: "materialização sem identidade do tipo concreto".to_string(),
                                span: expr.span,
                            })?;

                    let concrete_identity = lowered_source.identity(self.context, expr.span)?;
                    let concrete_size = self.concrete_snapshot_size(&lowered_source, expr.span)?;

                    let vtable_methods = self.trait_vtable(
                        &trait_name,
                        concrete_identity,
                        &concrete_type_name,
                        expr.span,
                    )?;

                    // A identidade do objeto de trato é `trato<Nome>`: dois
                    // tratos diferentes compartilham `TypeIR::TraitObject` e não
                    // podem colapsar na mesma identidade.
                    let trait_object_identity = self.context.resolved_identity(&Type::Applied {
                        name: "trato".to_string(),
                        args: vec![Type::Alias {
                            name: trait_name.clone(),
                            span: expr.span,
                        }],
                        span: expr.span,
                    })?;

                    return Ok(TypedValueIR {
                        value: ValueIR::MakeTraitObject {
                            value: Box::new(lowered_source.value),
                            trait_name,
                            concrete_type: lowered_source.ty,
                            concrete_type_name,
                            concrete_size,
                            vtable_methods,
                        },
                        ty: TypeIR::TraitObject,
                        resolved: Some(trait_object_identity),
                        ptr_array_bombom_size: None,
                    });
                }

                if let TypeIR::Union(union_type_id) = target_type {
                    // A identidade semântica exata do valor de origem é
                    // obrigatória: sem ela não há injeção possível, e escolher
                    // um membro por representação ou por primeira ocorrência é
                    // exatamente o defeito HR4.
                    let source_identity = lowered_source.identity(self.context, expr.span)?;
                    let member = {
                        let registry = self.context.union_registry.borrow();
                        let union = registry
                            .types
                            .iter()
                            .find(|union| union.id == union_type_id)
                            .ok_or_else(|| PinkerError::Ir {
                                msg: "injeção perdeu o registro da união".to_string(),
                                span: expr.span,
                            })?;
                        let mut exact = union
                            .members
                            .iter()
                            .filter(|member| member.resolved_type_id == source_identity);
                        let member = exact.next().cloned().ok_or_else(|| {
                            // Nenhum membro tem esta identidade. Se existe um
                            // membro com a mesma representação, o diagnóstico
                            // aponta a confusão entre categoria e identidade em
                            // vez de aceitar o candidato aproximado.
                            let operational = union
                                .members
                                .iter()
                                .any(|member| member.ty == lowered_source.ty);
                            let key = self
                                .context
                                .resolved_types
                                .borrow()
                                .key_of(source_identity)
                                .unwrap_or("<desconhecida>")
                                .to_string();
                            if operational {
                                PinkerError::Ir {
                                    msg: format!(
                                        "E-IR-UNION-MEMBER-IDENTITY-MISMATCH: a união {} possui \
                                         membro com a representação '{}', mas nenhum com a \
                                         identidade '{key}'",
                                        union_type_id.0,
                                        lowered_source.ty.name()
                                    ),
                                    span: expr.span,
                                }
                            } else {
                                PinkerError::Ir {
                                    msg: format!(
                                        "tipo fonte de identidade '{key}' não pertence à união {} \
                                         durante o lowering",
                                        union_type_id.0
                                    ),
                                    span: expr.span,
                                }
                            }
                        })?;
                        if let Some(duplicate) = exact.next() {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "E-IR-UNION-IDENTITY-DUPLICATE: a união {} tem a identidade \
                                     resolvida {} nas tags {} e {}",
                                    union_type_id.0, source_identity.0, member.tag, duplicate.tag
                                ),
                                span: expr.span,
                            });
                        }
                        member
                    };
                    return Ok(TypedValueIR {
                        value: ValueIR::UnionInject {
                            value: Box::new(lowered_source.value),
                            union_type_id,
                            // A tag é **copiada** do membro exato; nenhuma camada
                            // posterior torna a escolher membro.
                            tag: member.tag,
                            resolved_member_type_id: member.resolved_type_id,
                            canonical_member_key: member.canonical_member_key.clone(),
                            payload_type: member.ty,
                            payload_layout: member.payload_layout,
                        },
                        ty: target_type,
                        resolved: Some(self.context.repr_identity(target_type, expr.span)?),
                        ptr_array_bombom_size: None,
                    });
                }

                Ok(TypedValueIR {
                    value: ValueIR::Cast {
                        value: Box::new(lowered_source.value),
                        target_type,
                    },
                    ty: target_type,
                    // O cast continua sem fabricar proveniência; esta identidade
                    // descreve somente o tipo-alvo e permite que uma operação
                    // tipada posterior recupere o layout de `seta<T>`.
                    resolved: Some(self.context.resolved_identity(target)?),
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::SizeOfType { target } => {
                let layout = layout::layout_of_type(
                    target,
                    &self.context.type_aliases,
                    &self.context.struct_decls,
                )
                .map_err(|msg| PinkerError::Ir {
                    msg: format!("consulta de peso inválida na IR: {}", msg),
                    span: expr.span,
                })?;
                Ok(TypedValueIR {
                    value: ValueIR::Int(layout.size),
                    ty: TypeIR::Bombom,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::AlignOfType { target } => {
                let layout = layout::layout_of_type(
                    target,
                    &self.context.type_aliases,
                    &self.context.struct_decls,
                )
                .map_err(|msg| PinkerError::Ir {
                    msg: format!("consulta de alinhamento inválida na IR: {}", msg),
                    span: expr.span,
                })?;
                Ok(TypedValueIR {
                    value: ValueIR::Int(layout.align),
                    ty: TypeIR::Bombom,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
        }
    }

    // @pinker-nav:end ir.lowering.expressoes-valores

    // @pinker-nav:start ir.lowering.bindings-escopos
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Normalização de nomes-fonte em slots e gestão de escopos léxicos: `allocate_binding` gera `%nome#N` (contador por nome-fonte), registra o binding no escopo atual e coleta `LocalIR`; a resolução sobe a pilha de escopos; e os rótulos de bloco/laço são gerados aqui. Slots são nomes normalizados desta camada — não são SSA nem registradores físicos de máquina.
    fn allocate_binding(
        &mut self,
        source_name: &str,
        ty: TypeIR,
        resolved: Option<ResolvedTypeId>,
        ptr_array_bombom_size: Option<u64>,
        is_mut: Option<bool>,
    ) -> Result<BindingIR, PinkerError> {
        // O slot transporta a identidade que o chamador determinou. `None`
        // segue a convenção de [`TypedValueIR::resolved`]: a identidade é a da
        // própria representação e é internada sob demanda. A exigência de
        // identidade exata é cobrada nos pontos que a **consomem** — injeção em
        // união, concordância de ramos e acesso nominal —, e não na alocação do
        // slot, para que nenhuma dessas checagens possa ser satisfeita por uma
        // identidade fabricada só para preencher o campo.
        let slot_identity = resolved;

        let next = self
            .slot_counters
            .entry(source_name.to_string())
            .or_insert(0);
        let slot = format!("%{}#{}", source_name, *next);
        *next += 1;

        let binding = BindingIR {
            source_name: source_name.to_string(),
            slot: slot.clone(),
            ty,
            resolved: slot_identity,
        };

        self.scopes.last_mut().unwrap().insert(
            source_name.to_string(),
            BindingState {
                slot: slot.clone(),
                ty,
                resolved: slot_identity,
                ptr_array_bombom_size,
            },
        );

        if let Some(is_mut) = is_mut {
            self.locals.push(LocalIR {
                source_name: source_name.to_string(),
                slot,
                ty,
                resolved: slot_identity,
                is_mut,
            });
        }

        Ok(binding)
    }

    fn resolve_binding(&self, source_name: &str, span: Span) -> Result<BindingState, PinkerError> {
        self.resolve_existing_binding(source_name)
            .ok_or_else(|| PinkerError::Ir {
                msg: format!("lowering falhou ao resolver variável '{}'", source_name),
                span,
            })
    }

    fn resolve_existing_binding(&self, source_name: &str) -> Option<BindingState> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(source_name).cloned())
    }

    fn next_block_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.block_counter);
        self.block_counter += 1;
        label
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    // @pinker-nav:end ir.lowering.bindings-escopos
}

// @pinker-nav:start ir.lowering.constantes
// @pinker-nav:domain lowering
// @pinker-nav:layer ir
// @pinker-nav:summary Abaixa uma constante global: cria um `FunctionLowerer` mínimo para o inicializador, abaixa o valor e o tipo declarado e monta `ConstIR`. Consome o contexto já preparado; não valida o inicializador (a semântica já o fez).
pub(super) fn lower_const(
    const_decl: &ConstDecl,
    context: &LoweringContext,
) -> Result<ConstIR, PinkerError> {
    let mut lowerer = FunctionLowerer::new(context);
    let value = lowerer.lower_value(&const_decl.init)?;
    Ok(ConstIR {
        name: const_decl.name.clone(),
        ty: context.resolve_type(&const_decl.ty)?,
        value: value.value,
        span: const_decl.span,
    })
}
// @pinker-nav:end ir.lowering.constantes
