//! Montagem do contexto global de lowering e orquestração do programa na IR
//! estruturada, movidas de `src/ir.rs` pela unidade IR-2 do inventário da #601
//! (Task #624).
//!
//! Só o arquivo mudou: as cinco regiões cartografadas
//! `ir.lowering.programa-orquestracao`, `ir.lowering.contexto-declaracoes`,
//! `ir.lowering.assinaturas-intrinsecos`, `ir.lowering.metodos-identidade` e
//! `ir.lowering.identidade-resolvida` — a entrada pública do lowering, as duas
//! metades de `from_program_composto`, o registro da visão derivada de métodos
//! e a internação de identidade resolvida — chegam aqui na mesma ordem, com os
//! mesmos corpos e os mesmos diagnósticos. `super` mudou de significado ao
//! descer um nível, e o `use` abaixo devolve ao irmão o vocabulário do pai sem
//! promover nada: um filho enxerga os itens privados do pai por privacidade de
//! módulo, e este `use` é privado.
//!
//! O estado continua sendo do pai: `struct LoweringContext` segue declarada em
//! `src/ir.rs`, com os mesmos campos privados, e `resolve_type`,
//! `resolve_union_ast_type` e `intern_union` — que a IR-2 não move — continuam
//! no `impl` do pai. Este arquivo é implementação física da mesma fase de
//! lowering, não uma fase nova.
//!
//! As autoridades atravessadas continuam fora daqui. A seleção de método é de
//! `crate::method_dispatch` (#590/#591, consolidação C2): `register_impl_methods`
//! só pergunta qual função materializada representa a identidade e transporta o
//! veredito, sem regra própria de precedência, desempate ou grafia. As grafias
//! históricas de intrínseca continuam vindo do registry declarativo de
//! `crate::intrinsics::registry` (C1), sem censo local. A validação da IR
//! continua em `src/ir_validate.rs` e a fronteira de CFG continua em
//! `src/cfg_ir.rs`; nada disso desceu com o corte.
//!
//! `lower_program` e `lower_program_composto` já eram `pub` antes do move e
//! continuam `pub` aqui; o pai os reexporta para preservar
//! `pinker_v0::ir::lower_program` e `pinker_v0::ir::lower_program_composto`.
//! `resolved_identity`, `intern_resolved_ast`, `repr_identity` e
//! `internal_identity` são os quatro símbolos que o pai ou o irmão
//! `src/ir/lowering.rs` chamam e, por isso, os únicos que passaram de privados
//! a `pub(super)`.

use super::lowering::lower_const;
use super::*;

// Fase 2 escolhe IR estruturada: blocos e `if` seguem explícitos, sem SSA e sem saltos.
// Isso mantém o lowering pequeno e auditável sem quebrar o frontend estabilizado.
// @pinker-nav:start ir.lowering.programa-orquestracao
// @pinker-nav:domain lowering
// @pinker-nav:layer ir
// @pinker-nav:summary Ponto de entrada do lowering AST → IR: constrói o `LoweringContext` global, percorre os itens do programa, despacha constantes (`lower_const`) e funções (`FunctionLowerer`) e monta o `ProgramIR` (nome do módulo, modo freestanding). Aliases/structs/leques/tratos são ignorados aqui (já viraram fatos do contexto); não reexecuta análise semântica.
pub fn lower_program(program: &Program) -> Result<ProgramIR, PinkerError> {
    lower_program_composto(program, HashMap::new())
}

/// Abaixa um programa composto, respeitando o ambiente de cada unidade-fonte no
/// despacho não qualificado de método.
pub fn lower_program_composto(
    program: &Program,
    traits_visiveis_por_fonte: HashMap<SourceId, crate::module_resolve::TratosNoDespacho>,
) -> Result<ProgramIR, PinkerError> {
    let context = LoweringContext::from_program_composto(program, traits_visiveis_por_fonte)?;
    let mut consts = Vec::new();
    let mut functions = Vec::new();

    for item in &program.items {
        match item {
            Item::Const(const_decl) => consts.push(lower_const(const_decl, &context)?),
            Item::Function(function_decl)
                if context.ignored_impl_functions.contains(&function_decl.name) => {}
            // Fase 243: closures (`__anon_carinho_*`) são abaixadas lazily
            // no ponto de criação (`FunctionLowerer::resolve_closure`), com
            // o ambiente correto — não aqui, isoladas.
            Item::Function(function_decl)
                if crate::anonymous_identity::is_anonymous_callable_name(&function_decl.name) => {}
            Item::Function(function_decl) => {
                functions.push(FunctionLowerer::new(&context).lower_function(function_decl)?)
            }
            Item::TypeAlias(_) => {}
            Item::Struct(_) => {}
            Item::Enum(_) => {}
            Item::Trait(_) => {}
        }
    }

    // Fase 243: closure sintética nunca resolvida como valor (idioma de
    // chamada imediata `carinho(...) {...}(x)`, Fase 225) nunca passa por
    // `resolve_closure` — permanece função comum, sem `__env`, igual ao
    // comportamento anterior à Fase 243. Só closures genuinamente usadas
    // como valor recebem a convenção uniforme de ambiente.
    for item in &program.items {
        if let Item::Function(function_decl) = item {
            if crate::anonymous_identity::is_anonymous_callable_name(&function_decl.name) {
                let already = context
                    .closure_state
                    .borrow()
                    .captures
                    .contains_key(&function_decl.name);
                if !already {
                    let lowered = FunctionLowerer::new(&context).lower_function(function_decl)?;
                    let mut state = context.closure_state.borrow_mut();
                    state.lowered.push((function_decl.name.clone(), lowered));
                }
            }
        }
    }

    // A metadata das variantes é selada antes de a tabela de identidades ser
    // entregue: cada carga interna aqui a identidade do próprio tipo resolvido
    // e, quando é lista, a identidade concreta do elemento.
    let enum_variants_meta = context.seal_enum_variant_metadata()?;

    let ClosureLoweringState { lowered, .. } = context.closure_state.into_inner();
    functions.extend(lowered.into_iter().map(|(_, f)| f));
    let union_types = context.union_registry.into_inner().types;
    let resolved_types = context.resolved_types.into_inner().into_types();
    // A tabela de identidades e o registro de uniões são conferidos já aqui, no
    // ponto em que ambos ficam completos: qualquer incoerência é erro interno do
    // lowering e não deve chegar às camadas seguintes.
    let program_span = Span::new(Position::new(1, 1), Position::new(1, 1));
    validate_resolved_type_table(&resolved_types).map_err(|msg| PinkerError::Ir {
        msg: format!("E-IR-TYPE-IDENTITY-LOST: {msg}"),
        span: program_span,
    })?;
    validate_union_registry_identities(&union_types, &resolved_types).map_err(|msg| {
        PinkerError::Ir {
            msg,
            span: program_span,
        }
    })?;

    Ok(ProgramIR {
        module_name: context.module_name,
        is_freestanding: program.freestanding.is_some(),
        resolved_types,
        union_types,
        enum_variants: enum_variants_meta,
        consts,
        functions,
    })
}
// @pinker-nav:end ir.lowering.programa-orquestracao

impl LoweringContext {
    // @pinker-nav:start ir.lowering.contexto-declaracoes
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Primeira metade de `from_program`: coleta os fatos globais que todos os corpos consomem — nome do módulo, aliases de tipo (com leques registrados como alias para `bombom`), structs e seus campos/offsets de layout, variantes de leque com índices e cargas, e as assinaturas das funções e tipos das constantes declaradas no programa. Prepara o contexto; não reexecuta a checagem semântica.
    fn from_program_composto(
        program: &Program,
        traits_visiveis_por_fonte: HashMap<SourceId, crate::module_resolve::TratosNoDespacho>,
    ) -> Result<Self, PinkerError> {
        let module_name = program
            .package
            .as_ref()
            .map(|package| package.name.clone())
            .unwrap_or_else(|| "main".to_string());

        // Tabela de identidades semânticas do programa. Nasce aqui porque as
        // assinaturas das intrínsecas embutidas já precisam internar a
        // identidade do próprio retorno.
        let mut resolved_types = ResolvedTypeTable::default();
        let mut type_aliases = HashMap::new();
        let mut struct_decls = HashMap::new();
        let mut struct_names = HashSet::new();
        let mut enum_variants: HashMap<String, EnumInfoIR> = HashMap::new();
        let mut enum_decl_names: HashSet<String> = HashSet::new();
        // Tabelas exclusivas da classificação de cargas (D1). Não podem ser
        // `type_aliases`: ali um leque já foi reescrito para `bombom`, e usar
        // aquela tabela apagaria justamente a identidade nominal que a carga
        // precisa preservar.
        let mut payload_aliases: HashMap<String, Type> = HashMap::new();
        for item in &program.items {
            if let Item::TypeAlias(alias) = item {
                type_aliases.insert(alias.name.clone(), alias.target.clone());
                payload_aliases.insert(alias.name.clone(), alias.target.clone());
            } else if let Item::Struct(struct_decl) = item {
                struct_names.insert(struct_decl.name.clone());
                struct_decls.insert(struct_decl.name.clone(), struct_decl.clone());
            } else if let Item::Enum(enum_decl) = item {
                // O tipo leque abaixa para bombom na IR (discriminante imediato
                // ou handle); registrar como alias faz toda anotação de tipo
                // com o nome do leque resolver sozinha.
                type_aliases.insert(enum_decl.name.clone(), Type::Bombom(enum_decl.span));
                enum_decl_names.insert(enum_decl.name.clone());
            }
        }
        // As cargas são classificadas depois da coleta completa: um leque pode
        // referenciar outro declarado adiante, inclusive a si mesmo através de
        // `lista<si>`.
        for item in &program.items {
            let Item::Enum(enum_decl) = item else {
                continue;
            };
            let mut variants = HashMap::new();
            for (index, variant) in enum_decl.variants.iter().enumerate() {
                let mut payloads = Vec::with_capacity(variant.payloads.len());
                for payload in &variant.payloads {
                    payloads.push(EnumPayloadTypeIR::classify(
                        payload,
                        &payload_aliases,
                        &enum_decl_names,
                        &struct_names,
                        &type_aliases,
                    )?);
                }
                variants.insert(variant.name.clone(), (index as u64, payloads));
            }
            enum_variants.insert(
                enum_decl.name.clone(),
                EnumInfoIR {
                    declared_name: enum_decl.name.clone(),
                    has_payload: enum_decl
                        .variants
                        .iter()
                        .any(|variant| !variant.payloads.is_empty()),
                    variants,
                },
            );
        }
        // Apelidos de leque herdam a metadata do alvo. A propagação roda até o
        // ponto fixo porque a cadeia pode ter mais de um elo
        // (`apelido B = A; apelido A = Leque;`), e o `declared_name` de cada
        // entrada continua sendo o do leque real: o apelido não cria
        // identidade nominal nova.
        loop {
            let mut mudou = false;
            for (alias_name, target) in type_aliases.clone() {
                if enum_variants.contains_key(&alias_name) {
                    continue;
                }
                let (Type::Enum { name, .. } | Type::Alias { name, .. }) = target else {
                    continue;
                };
                if let Some(info) = enum_variants.get(&name).cloned() {
                    enum_variants.insert(alias_name, info);
                    mudou = true;
                }
            }
            if !mudou {
                break;
            }
        }
        let mut struct_fields = HashMap::new();
        let mut struct_field_offsets = HashMap::new();
        for item in &program.items {
            if let Item::Struct(struct_decl) = item {
                let mut fields = HashMap::new();
                for field in &struct_decl.fields {
                    let resolved =
                        TypeIR::from_ast_with_context(&field.ty, &type_aliases, &struct_names)?;
                    fields.insert(field.name.clone(), resolved);
                }
                struct_fields.insert(struct_decl.name.clone(), fields);
                let offsets =
                    layout::struct_field_offsets(&struct_decl.name, &type_aliases, &struct_decls)
                        .map_err(|msg| PinkerError::Ir {
                        msg: format!("layout de struct inválido na IR: {}", msg),
                        span: struct_decl.span,
                    })?;
                struct_field_offsets.insert(struct_decl.name.clone(), offsets);
            }
        }

        let mut traits = HashMap::new();

        for item in &program.items {
            let Item::Trait(trait_decl) = item else {
                continue;
            };

            let methods = trait_decl
                .methods
                .iter()
                .map(|method| {
                    let param_types = method
                        .params
                        .iter()
                        .skip(1)
                        .map(|param| {
                            TypeIR::from_ast_with_context(&param.ty, &type_aliases, &struct_names)
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    let ret_type = TypeIR::from_ast_option_with_context(
                        method.ret_type.as_ref(),
                        &type_aliases,
                        &struct_names,
                    )?;

                    let ret_trait_name = method
                        .ret_type
                        .as_ref()
                        .map(|ty| trait_object_name_from_type(ty, &type_aliases, &struct_names))
                        .transpose()?
                        .flatten();

                    Ok::<_, PinkerError>(TraitMethodMetaIR {
                        name: method.name.clone(),
                        param_types,
                        ret_type,
                        ret_ast: method.ret_type.clone(),
                        ret_trait_name,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            traits.insert(trait_decl.name.clone(), TraitMetaIR { methods });
        }

        let mut function_sigs = HashMap::new();
        // (nome, tipo AST do retorno, representação) das funções declaradas.
        let mut pending_declared_sigs: Vec<(String, Option<Type>, TypeIR)> = Vec::new();
        let mut global_consts = HashMap::new();
        let mut callable_metadata = HashMap::new();
        let mut raw_function_return_metadata = HashMap::new();
        let mut function_ret_pointer_pointees = HashMap::new();
        let mut function_ret_trait_names = HashMap::new();
        let mut all_functions = HashMap::new();

        for item in &program.items {
            match item {
                Item::Function(function) => {
                    all_functions.insert(function.name.clone(), function.clone());

                    if let Some(ret_type) = function.ret_type.as_ref() {
                        if let Some(trait_name) =
                            trait_object_name_from_type(ret_type, &type_aliases, &struct_names)?
                        {
                            function_ret_trait_names.insert(function.name.clone(), trait_name);
                        }
                    }

                    // A identidade semântica do retorno de uma função declarada
                    // pode exigir resolução integral de apelidos e internação de
                    // uniões, o que só é possível com o contexto já montado. A
                    // assinatura é registrada aqui apenas com a representação e
                    // é selada em `seal_declared_signature_identities`, antes de
                    // qualquer corpo ser abaixado.
                    pending_declared_sigs.push((
                        function.name.clone(),
                        function.ret_type.clone(),
                        TypeIR::from_ast_option_with_context(
                            function.ret_type.as_ref(),
                            &type_aliases,
                            &struct_names,
                        )?,
                    ));
                    // Fase 242: quando a função retorna um valor callable,
                    // registra o ret_type DESSE callable (um nível), para
                    // permitir chamada indireta imediata sobre o resultado.
                    if let Some(Type::Function { ret, .. }) = function.ret_type.as_ref() {
                        callable_metadata.insert(
                            function.name.clone(),
                            CallableMetadata {
                                ret_type: TypeIR::from_ast_with_context(
                                    ret,
                                    &type_aliases,
                                    &struct_names,
                                )?,
                                ret_ast: Some(ret.as_ref().clone()),
                                ret_trait_name: trait_object_name_from_type(
                                    ret,
                                    &type_aliases,
                                    &struct_names,
                                )?,
                                ret_pointer_pointee: pointer_pointee_from_type(
                                    ret,
                                    &type_aliases,
                                    &struct_names,
                                )?,
                            },
                        );
                    }
                    if let Some(ret_type) = function.ret_type.as_ref() {
                        if let Some(metadata) =
                            raw_function_metadata_from_type(ret_type, &type_aliases, &struct_names)?
                        {
                            raw_function_return_metadata.insert(function.name.clone(), metadata);
                        }
                        if let Some(pointee) =
                            pointer_pointee_from_type(ret_type, &type_aliases, &struct_names)?
                        {
                            function_ret_pointer_pointees.insert(function.name.clone(), pointee);
                        }
                    }
                }
                Item::Const(const_decl) => {
                    global_consts.insert(
                        const_decl.name.clone(),
                        TypeIR::from_ast_with_context(
                            &const_decl.ty,
                            &type_aliases,
                            &struct_names,
                        )?,
                    );
                }
                Item::TypeAlias(_) | Item::Struct(_) | Item::Enum(_) | Item::Trait(_) => {}
            }
        }
        // @pinker-nav:end ir.lowering.contexto-declaracoes

        // @pinker-nav:start ir.lowering.assinaturas-intrinsecos
        // @pinker-nav:domain lowering
        // @pinker-nav:layer ir
        // @pinker-nav:summary Segunda metade de `from_program`: assinaturas das intrínsecas que o lowering precisa tipar. As grafias históricas vêm do registry declarativo de `intrinsics::registry`, e as famílias falível, JSON, SHA-256 e acessores de processo vêm de suas próprias autoridades; o que continua declarado aqui são as identidades que o próprio compilador materializa. Encerra montando o `LoweringContext`. Não valida os corpos das intrínsecas; apenas declara contratos de retorno.
        // #442/C1 — as assinaturas históricas vêm do registry declarativo.
        //
        // Mesma disciplina já aplicada a `falha_operacional`, `valor_json`,
        // `sha256` e aos acessores de processo: a fase consulta a autoridade em
        // vez de manter a sua cópia da tabela.
        for entrada in crate::intrinsics::registry::HISTORICAL {
            let Some((retorno, _)) = entrada.assinatura_ir() else {
                continue;
            };
            let sig = match retorno {
                // `alocar` devolve `seta<u8>`: a identidade do apontado é
                // explícita, porque `TypeIR::Pointer` não a determina.
                TypeIR::Pointer { is_volatile } => {
                    let pointee = intern_representation_identity(&mut resolved_types, TypeIR::U8)
                        .map_err(|msg| PinkerError::Ir {
                        msg,
                        span: Span::new(Position::new(1, 1), Position::new(1, 1)),
                    })?;
                    let ret_type = TypeIR::Pointer { is_volatile };
                    let ret_resolved = resolved_types
                        .intern(
                            "ptr:0:u8".to_string(),
                            ret_type,
                            ResolvedTypeParts {
                                pointee: Some(pointee),
                                ..ResolvedTypeParts::default()
                            },
                        )
                        .map_err(|msg| PinkerError::Ir {
                            msg,
                            span: Span::new(Position::new(1, 1), Position::new(1, 1)),
                        })?;
                    FunctionSigIR {
                        ret_type,
                        ret_resolved,
                    }
                }
                outro => builtin_sig(&mut resolved_types, outro)?,
            };
            function_sigs.insert(entrada.spelling.to_string(), sig);
        }
        // U-01 — as operações internas com contrato fixo vêm da autoridade
        // declarativa; esta camada só precisa do retorno.
        //
        // `CARGA_SAIDA_PROCESSO` continua logo abaixo com assinatura NOMINAL:
        // o handle opaco não determina identidade semântica sozinho, e o tipo
        // nominal é fato de `falha_operacional`, não do contrato estrutural.
        for (spelling, ret_type, _params) in crate::internal_operations::assinaturas_declaradas() {
            if spelling == crate::enum_payload::CARGA_SAIDA_PROCESSO {
                continue;
            }
            function_sigs.insert(
                spelling.to_string(),
                builtin_sig(&mut resolved_types, ret_type)?,
            );
        }
        function_sigs.insert(
            crate::enum_payload::CARGA_SAIDA_PROCESSO.to_string(),
            builtin_nominal_sig(
                &mut resolved_types,
                crate::falha_operacional::CargaResultado::SaidaProcesso
                    .tipo(crate::falha_operacional::span_sintetico()),
            )?,
        );
        // As uniões não registram intrínsecas chamáveis: tag e extração são
        // `ValueIR::UnionTag`/`ValueIR::UnionExtract`, nós tipados criados pelo
        // lowering de `Stmt::UnionMatch`.
        // Parte B: leque com carga é handle de uma palavra na IR.
        for nome in crate::falha_operacional::nomes() {
            function_sigs.insert(
                nome.to_string(),
                builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
            );
        }
        // Parte E1: assinaturas derivadas da autoridade única de `valor_json`.
        //
        // Quem devolve `ValorJson` precisa de assinatura NOMINAL: a
        // representação `handle opaco` não determina identidade semântica
        // sozinha, e derivá-la da representação apagaria a diferença entre esta
        // família e qualquer outra que também seja uma palavra.
        for nome in crate::valor_json::ACESSORES {
            let (retorno, _) = crate::valor_json::assinatura_ir(nome)
                .expect("acessor JSON sem assinatura na autoridade");
            let sig = if matches!(retorno, TypeIR::OpaqueWordHandle) {
                builtin_nominal_sig(
                    &mut resolved_types,
                    Type::OpaqueHandle {
                        name: crate::valor_json::TIPO_VALOR_JSON.to_string(),
                        span: Span::new(Position::new(1, 1), Position::new(1, 1)),
                    },
                )?
            } else {
                builtin_sig(&mut resolved_types, retorno)?
            };
            function_sigs.insert(nome.to_string(), sig);
        }
        // Parte E2: a família SHA-256 devolve `verso`, então usa a assinatura
        // comum — nenhum tipo nominal novo entra na tabela por causa dela.
        for nome in crate::sha256::ACESSORES {
            let (retorno, _) = crate::sha256::assinatura_ir(nome)
                .expect("acessor SHA-256 sem assinatura na autoridade");
            let sig = builtin_sig(&mut resolved_types, retorno)?;
            function_sigs.insert(nome.to_string(), sig);
        }
        function_sigs.insert(
            crate::saida_processo::ACESSOR_CODIGO.to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        for nome in [
            crate::saida_processo::ACESSOR_SAIDA,
            crate::saida_processo::ACESSOR_ERRO,
        ] {
            function_sigs.insert(
                nome.to_string(),
                builtin_sig(&mut resolved_types, TypeIR::Verso)?,
            );
        }
        // Fase 140
        // Fase 137
        // Fase 138
        // Fase 139
        // Fase 158
        // Fase 160
        // Fase 161
        // Fase 165
        // Fase 166
        // Fase 163
        // Fase 164

        let mut context = Self {
            module_name,
            function_sigs,
            declared_sigs: HashMap::new(),
            global_consts,
            type_aliases,
            struct_decls,
            struct_names,
            struct_fields,
            struct_field_offsets,
            enum_variants,
            enum_decl_names,
            traits,
            impl_methods: BTreeMap::new(),
            fontes_das_relacoes: BTreeMap::new(),
            traits_visiveis_por_fonte,
            ignored_impl_functions: HashSet::new(),
            function_ret_trait_names,
            callable_metadata,
            raw_function_return_metadata,
            function_ret_pointer_pointees,
            all_functions,
            union_registry: std::cell::RefCell::new(UnionRegistryState::default()),
            resolved_types: std::cell::RefCell::new(resolved_types),
            closure_state: std::cell::RefCell::new(ClosureLoweringState::default()),
        };
        context.seal_declared_signature_identities(pending_declared_sigs)?;
        context.register_impl_methods(program)?;
        Ok(context)
    }
    // @pinker-nav:end ir.lowering.assinaturas-intrinsecos

    // @pinker-nav:start ir.lowering.metodos-identidade
    // @pinker-nav:domain tratos
    // @pinker-nav:layer lowering
    // @pinker-nav:summary Visão derivada de métodos aceita pela semântica: percorre funções provisórias, resolve integralmente o tipo-alvo declarado transportado em `ImplFunctionFacts` para `ResolvedTypeId` e indexa `MethodIdentity(trato, identidade resolvida, método)` até o símbolo transportado; chamadas e vtables consultam somente essa visão, sem decodificar spelling para decidir identidade; qual função materializada representa a identidade é dito por `method_dispatch`, e aqui fica só a mensagem do lowering.
    fn register_impl_methods(&mut self, program: &Program) -> Result<(), PinkerError> {
        // #577: a origem da relação vem do bloco `impl`, não do método. O span
        // do bloco é do arquivo que o escreveu; o do método pode ser corpo
        // default copiado de outra unidade.
        for impl_decl in &program.impls {
            let target = self.resolved_identity(&impl_decl.target_ty)?;
            self.fontes_das_relacoes
                .entry((impl_decl.trait_name.clone(), target))
                .or_insert(impl_decl.span.source);
        }
        let mut candidates: BTreeMap<MethodIdentity<ResolvedTypeId>, Vec<&FunctionDecl>> =
            BTreeMap::new();
        for item in &program.items {
            let Item::Function(function) = item else {
                continue;
            };
            let Some((trait_name, _target_spelling, method_name)) =
                method_identity::parse_provisional_function_name(&function.name)
            else {
                continue;
            };
            let impl_facts = function
                .impl_facts
                .as_ref()
                .ok_or_else(|| PinkerError::Ir {
                    msg: format!(
                        "lowering recebeu método '{}.{}' sem alvo declarado do impl",
                        trait_name, method_name
                    ),
                    span: function.span,
                })?;
            let target = self.resolved_identity(&impl_facts.target_ty)?;
            let identity = MethodIdentity::new(trait_name, target, method_name);
            candidates.entry(identity).or_default().push(function);
        }

        for (identity, mut candidates) in candidates {
            // A escolha do representante é de `method_dispatch`; aqui só sobra
            // a mensagem do lowering, que é da fase.
            let escolhido = match method_dispatch::select_representative(
                &mut candidates,
                |candidate| candidate.name.as_str(),
                |candidate| candidate.e_default_selecionado(),
            ) {
                RepresentativeSelection::Selected(index) => index,
                RepresentativeSelection::ExplicitConflict {
                    previous,
                    conflicting,
                } => {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "lowering recebeu identidade de método duplicada para '{}.{}' (símbolos '{}' e '{}')",
                            identity.trait_name,
                            identity.method_name,
                            candidates[previous].name,
                            candidates[conflicting].name
                        ),
                        span: candidates[conflicting].span,
                    });
                }
            };
            let selected = candidates[escolhido];
            for candidate in candidates {
                if candidate.name != selected.name && candidate.e_default_selecionado() {
                    self.ignored_impl_functions.insert(candidate.name.clone());
                }
            }
            self.impl_methods.insert(identity, selected.name.clone());
        }
        Ok(())
    }
    // @pinker-nav:end ir.lowering.metodos-identidade

    // @pinker-nav:start ir.lowering.identidade-resolvida
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Internação da identidade semântica no lowering: `resolved_identity` resolve apelidos em profundidade e interna a identidade completa do tipo AST; `intern_resolved_ast` interna primeiro componentes de containers, ponteiros, arrays, assinaturas e uniões; `repr_identity` cobre somente categorias cuja identidade é derivável da representação e recusa nominais com `E-IR-TYPE-IDENTITY-LOST`; `internal_identity` reserva identidades sintéticas. Nenhuma função deriva identidade nominal de `TypeIR::name()`, span ou ordem de mapa.
    /// Interna a identidade semântica completa de um tipo escrito na fonte.
    ///
    /// Apelidos são resolvidos integralmente antes da chave: `apelido X = Alfa`
    /// e `Alfa` produzem o mesmo `ResolvedTypeId`.
    pub(super) fn resolved_identity(&self, ty: &Type) -> Result<ResolvedTypeId, PinkerError> {
        let resolved = self.resolve_union_ast_type(ty, &mut Vec::new())?;
        self.intern_resolved_ast(&resolved, ty.span())
    }

    /// Interna a identidade de um tipo **já resolvido**, recursivamente.
    pub(super) fn intern_resolved_ast(
        &self,
        resolved: &Type,
        span: Span,
    ) -> Result<ResolvedTypeId, PinkerError> {
        let key = union_canon::canonical_type_key(resolved);
        if union_canon::is_poisoned_key(&key) {
            return Err(PinkerError::Ir {
                msg: format!(
                    "E-IR-TYPE-IDENTITY-LOST: identidade semântica perdida antes da internação ('{key}')"
                ),
                span,
            });
        }
        if let Some(existing) = self.resolved_types.borrow().id_of_key(&key) {
            return Ok(existing);
        }

        // Componentes primeiro: a internação do agregado nunca acontece com o
        // `RefCell` da tabela emprestado, para que a recursão seja segura.
        let mut parts = ResolvedTypeParts {
            nominal: union_canon::nominal_identity_of(resolved)
                .map(|(kind, name)| (NominalTypeKindIR::from_canon(kind), name)),
            ..ResolvedTypeParts::default()
        };
        match resolved {
            Type::Pointer { base, .. } => {
                parts.pointee = Some(self.intern_resolved_ast(base, span)?);
            }
            Type::Function { params, ret, .. } => {
                let mut param_ids = Vec::with_capacity(params.len());
                for param in params {
                    param_ids.push(self.intern_resolved_ast(param, span)?);
                }
                parts.signature = Some(ResolvedSignatureIR {
                    params: param_ids,
                    ret: self.intern_resolved_ast(ret, span)?,
                });
            }
            Type::FixedArray { element, .. } => {
                parts.element = Some(self.intern_resolved_ast(element, span)?);
            }
            // `lista<Leque>` carrega a identidade do elemento: sem isto, duas
            // listas de leques diferentes ficariam distintas apenas pela chave
            // canônica e idênticas em estrutura interna.
            Type::ListEnum { element, .. } => {
                parts.element = Some(self.intern_resolved_ast(
                    &Type::Enum {
                        name: element.clone(),
                        span,
                    },
                    span,
                )?);
            }
            // Mapas genéricos podem transportar leques sob a representação
            // operacional `bombom`; a identidade do valor precisa sobreviver
            // para usos diretos como scrutinee de `encaixe`.
            Type::Map { value, .. } => {
                parts.element = Some(self.intern_resolved_ast(value, span)?);
            }
            Type::Union { members, .. } => {
                let mut member_ids = Vec::with_capacity(members.len());
                for member in members {
                    member_ids.push(self.intern_resolved_ast(member, span)?);
                }
                parts.union_members = Some(member_ids);
            }
            _ => {}
        }

        let representation = match resolved {
            Type::Union { members, span } => self.intern_union(members, *span)?,
            other => TypeIR::from_ast_with_context(other, &self.type_aliases, &self.struct_names)?,
        };

        self.resolved_types
            .borrow_mut()
            .intern(key, representation, parts)
            .map_err(|msg| PinkerError::Ir { msg, span })
    }

    /// Sela a metadata publicada das variantes de `leque`.
    ///
    /// Roda depois de todo o lowering, quando a tabela de identidades já contém
    /// tudo o que os corpos internaram, e antes de a tabela ser entregue. Cada
    /// carga interna aqui a identidade do próprio tipo resolvido e, quando é
    /// `lista<E>`, a identidade concreta do elemento — as duas dimensões que a
    /// representação operacional sozinha não determina.
    ///
    /// A iteração é feita sobre os nomes **declarados**: `enum_variants` também
    /// é indexado pelos apelidos que apontam para cada leque, e um apelido não
    /// cria uma identidade nominal nova.
    fn seal_enum_variant_metadata(&self) -> Result<Vec<EnumVariantMetaIR>, PinkerError> {
        let mut declared: Vec<&EnumInfoIR> = Vec::new();
        for (name, info) in &self.enum_variants {
            if *name == info.declared_name {
                declared.push(info);
            }
        }
        declared.sort_by(|a, b| a.declared_name.cmp(&b.declared_name));

        let mut meta = Vec::new();
        for info in declared {
            let mut variants: Vec<(&String, &(u64, Vec<EnumPayloadTypeIR>))> =
                info.variants.iter().collect();
            variants.sort_by_key(|(_, (discriminant, _))| *discriminant);
            for (variant_name, (discriminant, payloads)) in variants {
                let mut payload_meta = Vec::with_capacity(payloads.len());
                for payload in payloads {
                    let span = payload.shape.resolved.span();
                    let resolved_type_id =
                        self.intern_resolved_ast(&payload.shape.resolved, span)?;
                    // Listas monomórficas (`bombom`/`verso`) já têm a
                    // identidade completa na própria chave/representação. Só
                    // `lista<Leque>` precisa publicar separadamente a
                    // identidade nominal do elemento, exatamente como a
                    // entrada internada acima.
                    let element_type_id = match &payload.shape.resolved {
                        Type::ListEnum { element, .. } => Some(self.intern_resolved_ast(
                            &Type::Enum {
                                name: element.clone(),
                                span,
                            },
                            span,
                        )?),
                        _ => None,
                    };
                    payload_meta.push(EnumPayloadMetaIR {
                        operational_type: payload.operational_type,
                        class: payload.shape.class,
                        canonical_key: payload.shape.canonical_key(),
                        resolved_type_id,
                        element_type_id,
                    });
                }
                meta.push(EnumVariantMetaIR {
                    enum_name: info.declared_name.clone(),
                    variant_name: variant_name.clone(),
                    discriminant: *discriminant,
                    payloads: payload_meta,
                });
            }
        }
        Ok(meta)
    }

    /// Identidade de um valor cuja categoria operacional **é** a identidade
    /// completa.
    ///
    /// Vale para escalares, `verso`, listas e mapas monomórficos, arrays de
    /// escalar, `nulo` e uniões já internadas. As categorias nominais ou
    /// paramétricas (`ninho`, `leque` — que também abaixa para escalar —,
    /// `seta<T>`, `carinho(...)`, `trato<...>`) **não** podem ser derivadas da
    /// representação e produzem `E-IR-TYPE-IDENTITY-LOST`.
    pub(super) fn repr_identity(
        &self,
        ty: TypeIR,
        span: Span,
    ) -> Result<ResolvedTypeId, PinkerError> {
        let lost = || {
            PinkerError::Ir {
            msg: format!(
                "E-IR-TYPE-IDENTITY-LOST: a representação '{}' não determina a identidade semântica",
                ty.name()
            ),
            span,
        }
        };
        let (key, parts) = match ty {
            TypeIR::Bombom
            | TypeIR::U8
            | TypeIR::U16
            | TypeIR::U32
            | TypeIR::U64
            | TypeIR::I8
            | TypeIR::I16
            | TypeIR::I32
            | TypeIR::I64
            | TypeIR::Logica
            | TypeIR::Verso
            | TypeIR::ListBombom
            | TypeIR::ListVerso
            | TypeIR::MapVersoBombom
            | TypeIR::MapVersoVerso
            | TypeIR::MapBombomBombom
            | TypeIR::MapBombomVerso
            | TypeIR::Nulo => (
                expected_key_for_representation(ty)
                    .ok_or_else(lost)?
                    .to_string(),
                ResolvedTypeParts::default(),
            ),
            TypeIR::FixedArray { element, size } => {
                let element_id = self.repr_identity(element.to_type_ir(), span)?;
                let element_key = {
                    let table = self.resolved_types.borrow();
                    table.key_of(element_id).ok_or_else(lost)?.to_string()
                };
                (
                    format!("array:{size}:{}:{element_key}", element_key.len()),
                    ResolvedTypeParts {
                        element: Some(element_id),
                        ..ResolvedTypeParts::default()
                    },
                )
            }
            TypeIR::Union(union_type_id) => {
                let member_keys = {
                    let registry = self.union_registry.borrow();
                    let union = registry
                        .types
                        .get(union_type_id.0 as usize)
                        .filter(|union| union.id == union_type_id)
                        .ok_or_else(lost)?;
                    union
                        .members
                        .iter()
                        .map(|member| {
                            (member.canonical_member_key.clone(), member.resolved_type_id)
                        })
                        .collect::<Vec<_>>()
                };
                let key = format!(
                    "union:[{}]",
                    member_keys
                        .iter()
                        .map(|(key, _)| format!("{}:{key}", key.len()))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                (
                    key,
                    ResolvedTypeParts {
                        union_members: Some(
                            member_keys.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
                        ),
                        ..ResolvedTypeParts::default()
                    },
                )
            }
            TypeIR::Struct
            | TypeIR::OpaqueWordHandle
            | TypeIR::Map { .. }
            | TypeIR::Pointer { .. }
            | TypeIR::FunctionPointer
            | TypeIR::Function
            | TypeIR::TraitObject => return Err(lost()),
        };
        self.resolved_types
            .borrow_mut()
            .intern(key, ty, parts)
            .map_err(|msg| PinkerError::Ir { msg, span })
    }

    /// Interna uma identidade sintética do próprio lowering (por exemplo o
    /// ambiente oculto `__env` das closures), com chave reservada que nunca
    /// coincide com um tipo escrito pelo usuário.
    pub(super) fn internal_identity(
        &self,
        tag: &str,
        ty: TypeIR,
        span: Span,
    ) -> Result<ResolvedTypeId, PinkerError> {
        // A chave reservada não é envenenada: é uma identidade legítima e
        // distinta, apenas inalcançável pela sintaxe de tipos do usuário.
        let mut parts = ResolvedTypeParts::default();
        let key = match ty {
            TypeIR::Pointer { is_volatile } => {
                let opaque = self
                    .resolved_types
                    .borrow_mut()
                    .intern(
                        format!("interno<{tag}>"),
                        TypeIR::Bombom,
                        ResolvedTypeParts::default(),
                    )
                    .map_err(|msg| PinkerError::Ir { msg, span })?;
                parts.pointee = Some(opaque);
                format!("ptr:{}:interno<{tag}>", u8::from(is_volatile))
            }
            _ => format!("interno<{tag}>"),
        };
        self.resolved_types
            .borrow_mut()
            .intern(key, ty, parts)
            .map_err(|msg| PinkerError::Ir { msg, span })
    }

    /// Sela a identidade semântica do retorno de cada função declarada.
    ///
    /// Roda depois de o contexto estar montado e antes de qualquer corpo ser
    /// abaixado, porque a identidade de um retorno pode exigir resolução
    /// integral de apelidos e internação de uniões — ambas dependentes do
    /// contexto completo. Nomes já ocupados pelo catálogo de intrínsecas
    /// embutidas continuam pertencendo às embutidas, exatamente como antes.
    fn seal_declared_signature_identities(
        &mut self,
        pending: Vec<(String, Option<Type>, TypeIR)>,
    ) -> Result<(), PinkerError> {
        let mut sealed = Vec::with_capacity(pending.len());
        for (name, ast_ret, ret_type) in pending {
            let span = ast_ret
                .as_ref()
                .map(|ty| ty.span())
                .unwrap_or_else(|| Span::new(Position::new(1, 1), Position::new(1, 1)));
            let ret_resolved = match ast_ret.as_ref() {
                Some(ty) => self.resolved_identity(ty)?,
                None => self.repr_identity(TypeIR::Nulo, span)?,
            };
            sealed.push((
                name,
                FunctionSigIR {
                    ret_type,
                    ret_resolved,
                },
            ));
        }
        for (name, sig) in sealed {
            self.declared_sigs.insert(name.clone(), sig.clone());
            self.function_sigs.entry(name).or_insert(sig);
        }
        Ok(())
    }
    // @pinker-nav:end ir.lowering.identidade-resolvida
}
