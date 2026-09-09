//! Módulos de teste do interpretador, movidos de `src/interpreter.rs` pela
//! unidade INT-1 do inventário da #601 — campanha INT-TESTS (Task #642).
//!
//! Só o arquivo mudou: as duas regiões cartografadas, os seis módulos e as
//! asserções continuam exatamente como estavam. `super` mudou de significado
//! ao descer um nível, e a reexportação abaixo é a ponte que devolve o pai aos
//! módulos que continuam escritos com `use super::*`. `mod tests` é privado e
//! `#[cfg(test)]`: a ponte não amplia superfície nenhuma para fora do módulo
//! `interpreter`.

pub use super::*;

#[cfg(test)]
mod fase244_trait_runtime_tests {
    use super::*;

    #[test]
    fn fase244_trait_runtime_snapshot_composto_independe_da_origem() {
        let mut state = TraitObjectState::new();
        let mut memory = HashMap::new();

        let source_addr = 0x4000usize;
        memory.insert(source_addr, RuntimeValue::Int(11));
        memory.insert(source_addr + 8, RuntimeValue::Int(22));

        let methods = vec!["__impl_7_Medivel_5_Ponto_medir".to_string()];

        let handle = state
            .create_object(
                RuntimeValue::Ptr(source_addr),
                "Medivel",
                crate::ir::TypeIR::Struct,
                "Ponto",
                16,
                &methods,
                &mut memory,
            )
            .unwrap();

        let (_, receiver) = state
            .resolve_call(handle, "Medivel", "medir", 0, &memory)
            .unwrap();

        let RuntimeValue::Ptr(snapshot_addr) = receiver else {
            panic!("receiver composto deveria ser ponteiro");
        };

        assert_ne!(snapshot_addr, source_addr);
        assert_eq!(memory.get(&snapshot_addr), Some(&RuntimeValue::Int(11)));
        assert_eq!(
            memory.get(&(snapshot_addr + 8)),
            Some(&RuntimeValue::Int(22))
        );

        memory.insert(source_addr, RuntimeValue::Int(99));
        memory.insert(source_addr + 8, RuntimeValue::Int(100));

        assert_eq!(memory.get(&snapshot_addr), Some(&RuntimeValue::Int(11)));
        assert_eq!(
            memory.get(&(snapshot_addr + 8)),
            Some(&RuntimeValue::Int(22))
        );
    }

    #[test]
    fn fase244_trait_runtime_vtable_e_internada_handles_sao_distintos() {
        let mut state = TraitObjectState::new();
        let mut memory = HashMap::new();
        let methods = vec!["__impl_7_Medivel_6_bombom_medir".to_string()];

        let first = state
            .create_object(
                RuntimeValue::Int(10),
                "Medivel",
                crate::ir::TypeIR::Bombom,
                "bombom",
                8,
                &methods,
                &mut memory,
            )
            .unwrap();

        let second = state
            .create_object(
                RuntimeValue::Int(20),
                "Medivel",
                crate::ir::TypeIR::Bombom,
                "bombom",
                8,
                &methods,
                &mut memory,
            )
            .unwrap();

        assert_ne!(first, second);

        let first_vtable = state.table.get(&first).unwrap().vtable_handle;
        let second_vtable = state.table.get(&second).unwrap().vtable_handle;

        assert_eq!(first_vtable, second_vtable);

        // Copiar o valor público copia apenas o handle e, portanto,
        // continua apontando para o mesmo descritor.
        let alias = first;
        assert_eq!(
            state.table.get(&alias).unwrap().data_addr,
            state.table.get(&first).unwrap().data_addr
        );
    }
}

#[cfg(test)]
mod d3_callable_lifetime_tests {
    use super::*;

    #[test]
    fn descritor_possui_ambiente_trailing_e_instancias_sao_independentes() {
        let mut state = CallableState::new();
        let mut memory = HashMap::new();

        let first = state
            .create_closure_instance(
                "closure",
                vec![RuntimeValue::Int(11), RuntimeValue::Int(22)],
                &mut memory,
            )
            .expect("primeira closure");
        let second = state
            .create_closure_instance(
                "closure",
                vec![RuntimeValue::Int(33), RuntimeValue::Int(44)],
                &mut memory,
            )
            .expect("segunda closure");

        let first_env = state.table[&first].env_addr.expect("ambiente 1");
        let second_env = state.table[&second].env_addr.expect("ambiente 2");
        assert_eq!(first_env, 0x1000_0000 + 16);
        assert_eq!(second_env, 0x1000_0000 + 32 + 16);
        assert_eq!(memory[&first_env], RuntimeValue::Int(11));
        assert_eq!(memory[&(first_env + 8)], RuntimeValue::Int(22));
        assert_eq!(memory[&second_env], RuntimeValue::Int(33));
        assert_eq!(memory[&(second_env + 8)], RuntimeValue::Int(44));

        let alias = first;
        assert_eq!(state.table[&alias].env_addr, Some(first_env));
    }

    #[test]
    fn falha_de_endereco_nao_publica_handle_nem_ambiente_parcial() {
        let mut state = CallableState::new();
        state.next_allocation_addr = usize::MAX - 7;
        let mut memory = HashMap::new();

        let error = state
            .create_closure_instance("closure", vec![RuntimeValue::Int(1)], &mut memory)
            .expect_err("layout deve exceder o espaço de endereços")
            .to_string();

        assert!(error.contains("E-RUNTIME-CALLABLE-ALLOCATION"), "{error}");
        assert!(state.table.is_empty(), "nenhum handle pode ser publicado");
        assert!(memory.is_empty(), "nenhum ambiente parcial pode permanecer");
        assert_eq!(state.next_handle, 1);
        assert_eq!(state.next_allocation_addr, usize::MAX - 7);
    }
}

#[cfg(test)]
mod fase246_public_memory_tests {
    use super::*;

    fn registrar_regiao(state: &mut PublicMemoryState, base: usize, size: usize, alive: bool) {
        state.budget.identity_count += 1;
        state.budget.lifetime_virtual_bytes += size as u64;
        state.budget.metadata_bytes += pinker_memory_contract::PUBLIC_METADATA_BYTES_PER_IDENTITY;
        if alive {
            state.budget.live_reserved_bytes += size as u64;
        }
        state.regions.push(PublicMemoryRegion {
            base,
            size,
            reserved: size,
            alive,
        });
    }

    #[test]
    fn liberar_endereco_reutilizado_escolhe_a_geracao_viva_mais_recente() {
        let mut state = PublicMemoryState::default();
        let base = 0x6000_0000;
        registrar_regiao(&mut state, base, 16, false);
        registrar_regiao(&mut state, base, 16, false);
        registrar_regiao(&mut state, base, 16, true);

        public_memory_free(&[RuntimeValue::Ptr(base)], &mut state)
            .expect("a terceira geração viva deve poder ser liberada");

        assert!(!state.regions[0].alive);
        assert!(!state.regions[1].alive);
        assert!(!state.regions[2].alive);
    }

    #[test]
    fn bytes_publicos_preservam_largura_aliasing_e_extensao() {
        let mut memory = HashMap::new();
        let base = 0x6000_1000;

        public_memory_store_bytes(
            &mut memory,
            base,
            TypeIR::U32,
            RuntimeValue::Int(0x1234_5678),
        )
        .expect("store u32");
        assert_eq!(
            public_memory_load_bytes(&memory, base, TypeIR::U8).expect("load u8"),
            RuntimeValue::Int(0x78)
        );
        assert_eq!(
            public_memory_load_bytes(&memory, base, TypeIR::U16).expect("load u16"),
            RuntimeValue::Int(0x5678)
        );

        public_memory_store_bytes(
            &mut memory,
            base + 4,
            TypeIR::I8,
            RuntimeValue::IntSigned(-128),
        )
        .expect("store i8");
        assert_eq!(
            public_memory_load_bytes(&memory, base + 4, TypeIR::I8).expect("load i8"),
            RuntimeValue::IntSigned(-128)
        );
        assert_eq!(
            public_memory_load_bytes(&memory, base + 4, TypeIR::U8).expect("load u8"),
            RuntimeValue::Int(128)
        );
        assert_eq!(
            public_memory_load_bytes(&memory, base + 8, TypeIR::U64).expect("zero u64"),
            RuntimeValue::Int(0)
        );
    }

    #[test]
    fn interpretador_contabiliza_paginas_e_libera_somente_bytes_vivos() {
        let mut state = PublicMemoryState::default();
        let IntrinsicCall::Done(Some(RuntimeValue::Ptr(first))) =
            public_memory_allocate(&[RuntimeValue::Int(1)], &mut state).expect("primeira alocação")
        else {
            panic!("alocar precisa devolver ponteiro");
        };
        assert_eq!(state.budget.identity_count, 1);
        assert_eq!(state.budget.live_reserved_bytes, 4096);
        assert_eq!(state.budget.lifetime_virtual_bytes, 4096);
        assert_eq!(
            state.budget.metadata_bytes,
            pinker_memory_contract::PUBLIC_METADATA_BYTES_PER_IDENTITY
        );

        public_memory_free(&[RuntimeValue::Ptr(first)], &mut state).expect("libera primeira");
        assert_eq!(state.budget.live_reserved_bytes, 0);
        let after_free = state.budget;
        assert!(public_memory_free(&[RuntimeValue::Ptr(first)], &mut state).is_err());
        assert_eq!(state.budget, after_free, "double free não altera orçamento");
        assert!(public_memory_free(&[RuntimeValue::Ptr(0x1234)], &mut state).is_err());
        assert_eq!(
            state.budget, after_free,
            "foreign free não altera orçamento"
        );

        let IntrinsicCall::Done(Some(RuntimeValue::Ptr(second))) =
            public_memory_allocate(&[RuntimeValue::Int(1)], &mut state).expect("segunda alocação")
        else {
            panic!("alocar precisa devolver ponteiro");
        };
        assert_ne!(first, second);
        assert_eq!(state.budget.identity_count, 2);
        assert_eq!(state.budget.live_reserved_bytes, 4096);
        assert_eq!(state.budget.lifetime_virtual_bytes, 8192);
    }
}

#[cfg(test)]
mod hr3_union_budget_tests {
    use super::*;

    /// HR3: o orçamento interpretado é equivalente ao nativo. Os tetos são
    /// testados na fronteira, sem materializar milhões de descritores.
    #[test]
    fn orcamento_de_bytes_de_payload_e_finito() {
        let mut state = UnionRuntimeState {
            total_payload_bytes: crate::union_payload::MAX_UNION_TOTAL_PAYLOAD_BYTES - 8,
            ..UnionRuntimeState::default()
        };
        state.charge(8).expect("exatamente no teto é aceito");
        let error = state
            .charge(1)
            .expect_err("um byte acima do teto é recusado")
            .to_string();
        assert!(error.contains("E-RUNTIME-UNION-PAYLOAD-BUDGET"), "{error}");
    }

    #[test]
    fn orcamento_de_metadata_e_finito() {
        let mut state = UnionRuntimeState {
            metadata_bytes: crate::union_payload::MAX_UNION_METADATA_BYTES,
            ..UnionRuntimeState::default()
        };
        let error = state
            .charge(8)
            .expect_err("metadata esgotada é recusada")
            .to_string();
        assert!(error.contains("E-RUNTIME-UNION-METADATA-BUDGET"), "{error}");
    }

    #[test]
    fn orcamento_usa_operacoes_checked() {
        let mut state = UnionRuntimeState {
            total_payload_bytes: u64::MAX,
            ..UnionRuntimeState::default()
        };
        let error = state
            .charge(1)
            .expect_err("overflow é recusado")
            .to_string();
        assert!(error.contains("overflow"), "{error}");
    }
}

// @pinker-nav:start interpreter.unioes.contabilidade-dominios
// @pinker-nav:domain unioes
// @pinker-nav:layer interpreter
// @pinker-nav:summary Matriz de contabilidade dos dois domínios de storage do interpretador — identidades públicas consumidas exclusivamente por `alocar` e domínio interno de união com descritores, bytes de payload e bindings de extração —, provando com limites reduzidos por configuração interna que construir e extrair uniões não reduz a capacidade pública, que o esgotamento de cada domínio produz diagnóstico próprio, que `liberar` recusa endereços internos e que liberar memória pública não altera o orçamento interno.
#[cfg(test)]
mod contabilidade_dominios_uniao_tests {
    use super::*;

    /// Contabilidade observada ao fim de uma execução hospedada.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Contabilidade {
        identidades_publicas: usize,
        descritores_uniao: usize,
        bytes_payload_uniao: u64,
        metadata_uniao: u64,
        bindings_uniao: usize,
        bytes_binding_uniao: u64,
    }

    fn compilar(source: &str) -> crate::abstract_machine::MachineProgram {
        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().expect("lexer");
        let mut parser = crate::parser::Parser::new(tokens);
        let ast = parser.parse().expect("parser");
        crate::semantic::check_program(&ast).expect("semantic");
        let ir_program = crate::ir::lower_program(&ast).expect("ir");
        crate::ir_validate::validate_program(&ir_program).expect("ir validate");
        let cfg = crate::cfg_ir::lower_program(&ir_program).expect("cfg");
        crate::cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
        let selected = crate::instr_select::lower_program(&cfg).expect("selected");
        crate::instr_select_validate::validate_program(&selected).expect("selected validate");
        let machine = crate::abstract_machine::lower_program(&selected).expect("machine");
        crate::abstract_machine_validate::validate_program(&machine).expect("machine validate");
        machine
    }

    /// Executa com limites reduzidos por **configuração interna de teste**.
    ///
    /// Os limites viajam pelos mesmos campos usados em produção; não há variável
    /// de ambiente nem opção pública, e o comportamento não muda entre debug e
    /// release.
    fn executar(
        source: &str,
        publicos: PublicMemoryLimits,
        descritores: UnionBudgetLimits,
        bindings: UnionBindingLimits,
    ) -> (Result<RunOutcome, PinkerError>, Contabilidade) {
        let program = compilar(source);
        UNION_RUNTIME_STATE.with(|state| {
            *state.borrow_mut() = UnionRuntimeState {
                limits: descritores,
                ..UnionRuntimeState::default()
            };
        });
        let mut estado = PublicMemoryState {
            limits: publicos,
            union_bindings: UnionBindingArena {
                limits: bindings,
                ..UnionBindingArena::default()
            },
            ..PublicMemoryState::default()
        };
        let resultado = run_program_com_estado(&program, &[], &mut estado);
        let (descritores_uniao, bytes_payload_uniao, metadata_uniao) =
            UNION_RUNTIME_STATE.with(|state| {
                let state = state.borrow();
                (
                    state.descriptors.len(),
                    state.total_payload_bytes,
                    state.metadata_bytes,
                )
            });
        let contabilidade = Contabilidade {
            identidades_publicas: estado.regions.len(),
            descritores_uniao,
            bytes_payload_uniao,
            metadata_uniao,
            bindings_uniao: estado.union_bindings.regions.len(),
            bytes_binding_uniao: estado.union_bindings.bytes,
        };
        (resultado, contabilidade)
    }

    /// Execução com os limites canônicos de produção.
    fn executar_canonico(source: &str) -> (Result<RunOutcome, PinkerError>, Contabilidade) {
        executar(
            source,
            PUBLIC_MEMORY_LIMITS,
            UNION_BUDGET_LIMITS,
            UNION_BINDING_LIMITS,
        )
    }

    fn diagnostico(resultado: &Result<RunOutcome, PinkerError>) -> String {
        match resultado {
            Ok(_) => String::new(),
            Err(erro) => erro.to_string(),
        }
    }

    /// `n` chamadas de `alocar` e nenhuma união.
    fn fonte_somente_alocar(n: usize) -> String {
        let mut corpo = String::new();
        for indice in 0..n {
            corpo.push_str(&format!(
                "    nova p{indice}: seta<u8> = alocar(16);\n    *p{indice} = 1;\n"
            ));
        }
        format!("pacote main;\ntrazer memoria.alocar;\ncarinho principal() -> bombom {{\n{corpo}    mimo 0;\n}}\n")
    }

    /// `n` construções e extrações de união com payload **escalar**.
    fn fonte_uniao_escalar(n: u64) -> String {
        format!(
            "pacote main;\ntrazer memoria.alocar;\n\
             carinho principal() -> bombom {{\n\
             \x20   nova muda i: bombom = 0;\n\
             \x20   sempre que i < {n} {{\n\
             \x20       nova v: uniao<u8, verso> = (7 virar u8) virar uniao<u8, verso>;\n\
             \x20       encaixe v {{\n\
             \x20           caso u8(numero) {{ nova d: bombom = numero virar bombom; }}\n\
             \x20           caso verso(texto) {{ nova e: bombom = 0; }}\n\
             \x20       }}\n\
             \x20       i = i + 1;\n\
             \x20   }}\n\
             \x20   mimo 0;\n\
             }}\n"
        )
    }

    /// `n` construções e extrações de união com payload **agregado** de
    /// `[bombom; 2]` (16 bytes, multi-palavra), a partir de uma única região
    /// pública de origem.
    fn fonte_uniao_agregada(n: u64) -> String {
        format!(
            "pacote main;\ntrazer memoria.alocar;\n\
             carinho principal() -> bombom {{\n\
             \x20   nova base: seta<[bombom; 2]> = alocar(16) virar seta<[bombom; 2]>;\n\
             \x20   nova celula: seta<bombom> = base virar seta<bombom>;\n\
             \x20   *celula = 5;\n\
             \x20   nova muda i: bombom = 0;\n\
             \x20   sempre que i < {n} {{\n\
             \x20       nova v: uniao<[bombom; 2], u8> = (*base) virar uniao<[bombom; 2], u8>;\n\
             \x20       encaixe v {{\n\
             \x20           caso [bombom; 2](agregado) {{ nova d: bombom = agregado[0]; }}\n\
             \x20           caso u8(numero) {{ nova e: bombom = 0; }}\n\
             \x20       }}\n\
             \x20       i = i + 1;\n\
             \x20   }}\n\
             \x20   mimo 0;\n\
             }}\n"
        )
    }

    // -----------------------------------------------------------------------
    // Contrato da identidade pública
    // -----------------------------------------------------------------------

    #[test]
    fn alocar_consome_exatamente_uma_identidade_publica_por_chamada() {
        for chamadas in [0usize, 1, 3, 7] {
            let (resultado, conta) = executar_canonico(&fonte_somente_alocar(chamadas));
            resultado.expect("execução sem união deve concluir");
            assert_eq!(
                conta.identidades_publicas, chamadas,
                "cada `alocar` bem-sucedido consome exatamente uma identidade pública"
            );
            assert_eq!(conta.descritores_uniao, 0);
            assert_eq!(conta.bindings_uniao, 0);
        }
    }

    #[test]
    fn uniao_escalar_nao_consome_identidade_publica() {
        let (resultado, conta) = executar_canonico(&fonte_uniao_escalar(5));
        resultado.expect("execução com uniões escalares deve concluir");
        assert_eq!(
            conta.identidades_publicas, 0,
            "payload escalar não materializa storage endereçável"
        );
        assert_eq!(conta.descritores_uniao, 5);
        assert_eq!(
            conta.bytes_payload_uniao, 5,
            "u8 ocupa 1 byte por descritor"
        );
        assert_eq!(conta.bindings_uniao, 0);
        assert_eq!(conta.bytes_binding_uniao, 0);
    }

    #[test]
    fn uniao_agregada_nao_consome_identidade_publica() {
        for construcoes in [1u64, 3, 16] {
            let (resultado, conta) = executar_canonico(&fonte_uniao_agregada(construcoes));
            resultado.expect("execução com uniões agregadas deve concluir");
            // A única identidade pública é a origem criada por `alocar`.
            assert_eq!(
                conta.identidades_publicas, 1,
                "construir e extrair {construcoes} uniões agregadas não pode consumir identidade \
                 pública além do `alocar` da origem"
            );
            assert_eq!(conta.descritores_uniao, construcoes as usize);
            assert_eq!(conta.bytes_payload_uniao, construcoes * 16);
            assert_eq!(
                conta.bindings_uniao, construcoes as usize,
                "cada extração agregada materializa um binding no domínio interno"
            );
            assert_eq!(conta.bytes_binding_uniao, construcoes * 16);
        }
    }

    #[test]
    fn construir_unioes_nao_altera_a_capacidade_restante_de_alocar() {
        let (_, sem_uniao) = executar_canonico(&fonte_somente_alocar(1));
        let (_, com_uniao) = executar_canonico(&fonte_uniao_agregada(32));
        assert_eq!(
            sem_uniao.identidades_publicas, com_uniao.identidades_publicas,
            "32 construções agregadas não podem mudar a contagem de identidades públicas"
        );
    }

    // -----------------------------------------------------------------------
    // Esgotamento de cada domínio, com limites reduzidos
    // -----------------------------------------------------------------------

    #[test]
    fn esgotamento_publico_acontece_apenas_por_alocar() {
        let publicos = PublicMemoryLimits {
            max_identities: 3,
            ..PUBLIC_MEMORY_LIMITS
        };
        // Três `alocar` cabem exatamente na cota.
        let (ok, conta) = executar(
            &fonte_somente_alocar(3),
            publicos,
            UNION_BUDGET_LIMITS,
            UNION_BINDING_LIMITS,
        );
        ok.expect("três alocações cabem na cota de três identidades");
        assert_eq!(conta.identidades_publicas, 3);

        // A quarta falha, e só ela.
        let (erro, _) = executar(
            &fonte_somente_alocar(4),
            publicos,
            UNION_BUDGET_LIMITS,
            UNION_BINDING_LIMITS,
        );
        assert!(
            diagnostico(&erro).contains("limite de identidades públicas esgotado"),
            "{}",
            diagnostico(&erro)
        );
    }

    #[test]
    fn dominio_interno_de_uniao_nao_esgota_a_cota_publica() {
        // Cota pública mínima viável: apenas a origem agregada.
        let publicos = PublicMemoryLimits {
            max_identities: 1,
            ..PUBLIC_MEMORY_LIMITS
        };
        let (resultado, conta) = executar(
            &fonte_uniao_agregada(64),
            publicos,
            UNION_BUDGET_LIMITS,
            UNION_BINDING_LIMITS,
        );
        resultado.expect(
            "64 construções agregadas devem concluir mesmo com a cota pública inteira consumida \
             pela origem",
        );
        assert_eq!(conta.identidades_publicas, 1);
        assert_eq!(conta.bindings_uniao, 64);
    }

    #[test]
    fn esgotar_o_dominio_interno_mantem_a_cota_publica_numericamente_intacta() {
        let bindings = UnionBindingLimits {
            max_regions: 4,
            ..UNION_BINDING_LIMITS
        };
        let (erro, conta) = executar(
            &fonte_uniao_agregada(8),
            PUBLIC_MEMORY_LIMITS,
            UNION_BUDGET_LIMITS,
            bindings,
        );
        assert!(
            diagnostico(&erro).contains("E-RUNTIME-UNION-BINDING-BUDGET"),
            "{}",
            diagnostico(&erro)
        );
        assert_eq!(
            conta.identidades_publicas, 1,
            "o esgotamento interno não pode ter consumido identidade pública alguma além da origem"
        );
        assert_eq!(conta.bindings_uniao, 4, "o teto interno foi respeitado");
    }

    #[test]
    fn limite_de_descritores_internos_tem_diagnostico_proprio() {
        let descritores = UnionBudgetLimits {
            max_descriptors: 3,
            ..UNION_BUDGET_LIMITS
        };
        let (erro, conta) = executar(
            &fonte_uniao_agregada(8),
            PUBLIC_MEMORY_LIMITS,
            descritores,
            UNION_BINDING_LIMITS,
        );
        let mensagem = diagnostico(&erro);
        assert!(
            mensagem.contains("E-RUNTIME-UNION-DESCRIPTOR-BUDGET"),
            "{mensagem}"
        );
        assert!(
            !mensagem.contains("identidades públicas"),
            "o limite interno não pode reutilizar a mensagem do limite público: {mensagem}"
        );
        assert_eq!(conta.identidades_publicas, 1);
        assert_eq!(conta.descritores_uniao, 3);
    }

    #[test]
    fn limite_de_bytes_internos_tem_diagnostico_proprio() {
        // Cabem exatamente três payloads de 16 bytes.
        let descritores = UnionBudgetLimits {
            max_payload_bytes: 48,
            ..UNION_BUDGET_LIMITS
        };
        let (erro, conta) = executar(
            &fonte_uniao_agregada(8),
            PUBLIC_MEMORY_LIMITS,
            descritores,
            UNION_BINDING_LIMITS,
        );
        let mensagem = diagnostico(&erro);
        assert!(
            mensagem.contains("E-RUNTIME-UNION-PAYLOAD-BUDGET"),
            "{mensagem}"
        );
        assert!(!mensagem.contains("identidades públicas"), "{mensagem}");
        assert_eq!(conta.identidades_publicas, 1);
        assert_eq!(conta.bytes_payload_uniao, 48);
    }

    #[test]
    fn limite_de_bytes_de_binding_tem_diagnostico_proprio() {
        let bindings = UnionBindingLimits {
            max_bytes: 32,
            ..UNION_BINDING_LIMITS
        };
        let (erro, conta) = executar(
            &fonte_uniao_agregada(8),
            PUBLIC_MEMORY_LIMITS,
            UNION_BUDGET_LIMITS,
            bindings,
        );
        let mensagem = diagnostico(&erro);
        assert!(
            mensagem.contains("E-RUNTIME-UNION-BINDING-BYTES"),
            "{mensagem}"
        );
        assert!(!mensagem.contains("identidades públicas"), "{mensagem}");
        assert_eq!(conta.identidades_publicas, 1);
        assert_eq!(conta.bytes_binding_uniao, 32);
    }

    // -----------------------------------------------------------------------
    // Teste cruzado dos dois orçamentos
    // -----------------------------------------------------------------------

    #[test]
    fn cruzado_cota_publica_intacta_apos_muitas_unioes_agregadas() {
        // Cota de 4: a origem mais três alocações públicas restantes.
        let publicos = PublicMemoryLimits {
            max_identities: 4,
            ..PUBLIC_MEMORY_LIMITS
        };
        let cenario = |extras: usize, unioes: u64| -> String {
            let mut corpo = String::from(
                "    nova base: seta<[bombom; 2]> = alocar(16) virar seta<[bombom; 2]>;\n\
                 \x20   nova celula: seta<bombom> = base virar seta<bombom>;\n\
                 \x20   *celula = 5;\n",
            );
            corpo.push_str(&format!(
                "    nova muda i: bombom = 0;\n\
                 \x20   sempre que i < {unioes} {{\n\
                 \x20       nova v: uniao<[bombom; 2], u8> = (*base) virar uniao<[bombom; 2], u8>;\n\
                 \x20       encaixe v {{\n\
                 \x20           caso [bombom; 2](agregado) {{ nova d: bombom = agregado[0]; }}\n\
                 \x20           caso u8(numero) {{ nova e: bombom = 0; }}\n\
                 \x20       }}\n\
                 \x20       i = i + 1;\n\
                 \x20   }}\n"
            ));
            for indice in 0..extras {
                corpo.push_str(&format!("    nova extra{indice}: seta<u8> = alocar(16);\n"));
            }
            format!("pacote main;\ntrazer memoria.alocar;\ncarinho principal() -> bombom {{\n{corpo}    mimo 0;\n}}\n")
        };

        // 1 (origem) + 3 extras = 4, exatamente a cota, depois de 128 uniões.
        let (ok, conta) = executar(
            &cenario(3, 128),
            publicos,
            UNION_BUDGET_LIMITS,
            UNION_BINDING_LIMITS,
        );
        ok.expect("a última alocação pública permitida deve ser aceita depois das uniões");
        assert_eq!(conta.identidades_publicas, 4);
        assert_eq!(conta.bindings_uniao, 128);

        // Só a seguinte falha.
        let (erro, _) = executar(
            &cenario(4, 128),
            publicos,
            UNION_BUDGET_LIMITS,
            UNION_BINDING_LIMITS,
        );
        assert!(
            diagnostico(&erro).contains("limite de identidades públicas esgotado"),
            "{}",
            diagnostico(&erro)
        );
    }

    // -----------------------------------------------------------------------
    // Fronteira entre os domínios
    // -----------------------------------------------------------------------

    #[test]
    fn liberar_recusa_endereco_do_dominio_interno_de_uniao() {
        let mut estado = PublicMemoryState::default();
        let base = union_reserve_binding_storage(&mut estado, 16, 8).expect("binding reservado");
        assert!(
            base >= UNION_BINDING_BASE,
            "o binding deve viver na arena interna: {base:#x}"
        );
        assert!(
            estado.regions.is_empty(),
            "o binding não pode aparecer no registro de identidades públicas"
        );
        let Err(erro) = public_memory_free(&[RuntimeValue::Ptr(base)], &mut estado) else {
            panic!("'liberar' deve recusar um endereço interno");
        };
        let erro = erro.to_string();
        assert!(erro.contains("E-RUNTIME-MEM-FOREIGN-FREE"), "{erro}");
    }

    #[test]
    fn liberar_memoria_publica_nao_altera_o_orcamento_interno_de_unioes() {
        let mut estado = PublicMemoryState::default();
        let IntrinsicCall::Done(Some(RuntimeValue::Ptr(publico))) =
            public_memory_allocate(&[RuntimeValue::Int(16)], &mut estado).expect("alocar")
        else {
            panic!("'alocar' deveria devolver um ponteiro");
        };
        union_reserve_binding_storage(&mut estado, 16, 8).expect("binding reservado");
        let antes = (
            estado.union_bindings.regions.len(),
            estado.union_bindings.bytes,
            estado.union_bindings.next_address,
        );
        public_memory_free(&[RuntimeValue::Ptr(publico)], &mut estado).expect("liberar");
        let depois = (
            estado.union_bindings.regions.len(),
            estado.union_bindings.bytes,
            estado.union_bindings.next_address,
        );
        assert_eq!(
            antes, depois,
            "liberar memória pública não pode devolver nem alterar orçamento interno de uniões"
        );
    }

    #[test]
    fn arena_interna_e_disjunta_da_arena_publica() {
        let fim_publico = PUBLIC_MEMORY_BASE + PUBLIC_MEMORY_MAX_VIRTUAL_BYTES;
        assert!(
            UNION_BINDING_BASE >= fim_publico,
            "a arena interna deve começar depois do fim da arena pública: {UNION_BINDING_BASE:#x} \
             vs {fim_publico:#x}"
        );
    }

    #[test]
    fn bindings_sucessivos_recebem_regioes_distintas_e_alinhadas() {
        let mut estado = PublicMemoryState::default();
        let primeiro = union_reserve_binding_storage(&mut estado, 24, 8).expect("primeiro");
        let segundo = union_reserve_binding_storage(&mut estado, 24, 8).expect("segundo");
        assert_ne!(primeiro, segundo, "duas extrações não compartilham storage");
        assert_eq!(primeiro % 16, 0, "storage alinhado a 16");
        assert_eq!(segundo % 16, 0, "storage alinhado a 16");
        assert!(
            segundo >= primeiro + 24,
            "as regiões não podem se sobrepor: {primeiro:#x} e {segundo:#x}"
        );
        assert_eq!(estado.union_bindings.bytes, 48);
        assert_eq!(estado.regions.len(), 0);
    }

    #[test]
    fn alinhamento_acima_do_teto_e_recusado_antes_de_reservar() {
        let mut estado = PublicMemoryState::default();
        let erro = union_reserve_binding_storage(
            &mut estado,
            16,
            crate::union_payload::MAX_UNION_PAYLOAD_ALIGN * 2,
        )
        .expect_err("alinhamento acima do teto deve ser recusado")
        .to_string();
        assert!(erro.contains("E-RUNTIME-UNION-ALIGN"), "{erro}");
        assert_eq!(estado.union_bindings.regions.len(), 0, "nada foi reservado");
        assert_eq!(estado.union_bindings.bytes, 0);
    }

    #[test]
    fn orcamento_de_binding_usa_operacoes_checked() {
        let mut estado = PublicMemoryState {
            union_bindings: UnionBindingArena {
                bytes: u64::MAX,
                ..UnionBindingArena::default()
            },
            ..PublicMemoryState::default()
        };
        let erro = union_reserve_binding_storage(&mut estado, 1, 8)
            .expect_err("overflow deve ser recusado")
            .to_string();
        assert!(erro.contains("E-RUNTIME-UNION-BINDING-OVERFLOW"), "{erro}");
    }

    /// Alias transparente, união aninhada achatada e travessia por chamada
    /// direta: nenhuma dessas formas muda a contagem de identidades públicas.
    const FORMAS_COMPOSTAS: &str = r#"pacote main;
trazer memoria.alocar;

apelido Trio = [bombom; 3];

carinho consome(v: uniao<Trio, u8>) -> bombom {
    nova muda saida: bombom = 0;
    encaixe v {
        caso Trio(agregado) { saida = agregado[0]; }
        caso u8(numero) { saida = 0; }
    }
    mimo saida;
}

carinho principal() -> bombom {
    nova base: seta<Trio> = alocar(24) virar seta<Trio>;
    nova celula: seta<bombom> = base virar seta<bombom>;
    *celula = 55;

    nova valor: uniao<Trio, u8> = (*base) virar uniao<Trio, u8>;
    falar(consome(valor));

    nova aninhada: uniao<uniao<Trio, u8>, verso> = (*base) virar uniao<uniao<Trio, u8>, verso>;
    encaixe aninhada {
        caso Trio(a) { falar(a[0]); }
        caso u8(n) { falar(1); }
        caso verso(t) { falar(2); }
    }
    mimo 0;
}
"#;

    #[test]
    fn alias_uniao_aninhada_e_chamada_direta_nao_mudam_a_contagem() {
        let (resultado, conta) = executar_canonico(FORMAS_COMPOSTAS);
        resultado.expect("alias, união aninhada e chamada direta devem concluir");
        assert_eq!(
            conta.identidades_publicas, 1,
            "apelidos, achatamento de união aninhada e travessia por chamada não criam identidade \
             pública: só o `alocar` da origem conta"
        );
        // Duas injeções (uma por `virar`) e duas extrações agregadas.
        assert_eq!(conta.descritores_uniao, 2);
        assert_eq!(conta.bindings_uniao, 2);
        assert_eq!(conta.bytes_binding_uniao, 48, "dois agregados de 24 bytes");
    }

    #[test]
    fn extracao_e_reinjecao_nao_duplicam_consumo_indevido() {
        let fonte = r#"pacote main;
trazer memoria.alocar;
carinho principal() -> bombom {
    nova base: seta<[bombom; 2]> = alocar(16) virar seta<[bombom; 2]>;
    nova celula: seta<bombom> = base virar seta<bombom>;
    *celula = 31;
    nova primeira: uniao<[bombom; 2], u8> = (*base) virar uniao<[bombom; 2], u8>;
    encaixe primeira {
        caso [bombom; 2](agregado) {
            nova segunda: uniao<[bombom; 2], u8> = agregado virar uniao<[bombom; 2], u8>;
            encaixe segunda {
                caso [bombom; 2](copia) { falar(copia[0]); }
                caso u8(numero) { falar(999); }
            }
        }
        caso u8(numero) { falar(999); }
    }
    mimo 0;
}
"#;
        let (resultado, conta) = executar_canonico(fonte);
        resultado.expect("extração seguida de reinjeção deve concluir");
        assert_eq!(
            conta.identidades_publicas, 1,
            "reinjetar um binding não cria identidade pública"
        );
        assert_eq!(
            conta.descritores_uniao, 2,
            "duas injeções, duas cobranças de descritor — nem mais, nem menos"
        );
        assert_eq!(
            conta.bindings_uniao, 2,
            "duas extrações, dois bindings — a reinjeção não cobra um terceiro"
        );
        assert_eq!(conta.bytes_payload_uniao, 32);
        assert_eq!(conta.bytes_binding_uniao, 32);
    }

    #[test]
    fn binding_preserva_alinhamentos_de_1_a_16() {
        for align in [1u64, 2, 4, 8, 16] {
            let mut estado = PublicMemoryState::default();
            let base = union_reserve_binding_storage(&mut estado, align.max(1), align)
                .unwrap_or_else(|erro| panic!("alinhamento {align} deveria ser aceito: {erro}"));
            assert_eq!(
                base % (align as usize),
                0,
                "storage de binding desalinhado para align={align}: {base:#x}"
            );
            assert_eq!(estado.regions.len(), 0, "nenhuma identidade pública gasta");
        }
    }

    #[test]
    fn payload_multi_palavra_consome_os_bytes_corretos() {
        for palavras in [1u64, 2, 3, 8] {
            let bytes = palavras * 8;
            let mut estado = PublicMemoryState::default();
            union_reserve_binding_storage(&mut estado, bytes, 8).expect("binding multi-palavra");
            assert_eq!(
                estado.union_bindings.bytes, bytes,
                "um agregado de {palavras} palavras deve consumir {bytes} bytes internos"
            );
            assert_eq!(estado.regions.len(), 0);
        }
    }

    #[test]
    fn os_limites_dos_dois_dominios_sao_finitos_e_independentes() {
        // Finitos: cada teto é diferente de zero e de `usize`/`u64::MAX`.
        assert_ne!(PUBLIC_MEMORY_LIMITS.max_identities, 0);
        assert_ne!(PUBLIC_MEMORY_LIMITS.max_identities, usize::MAX);
        assert_ne!(UNION_BUDGET_LIMITS.max_descriptors, 0);
        assert_ne!(UNION_BUDGET_LIMITS.max_descriptors, u64::MAX);
        assert_ne!(UNION_BINDING_LIMITS.max_regions, 0);
        assert_ne!(UNION_BINDING_LIMITS.max_regions, u64::MAX);
        assert_ne!(UNION_BINDING_LIMITS.max_bytes, 0);
        assert_ne!(UNION_BINDING_LIMITS.max_bytes, u64::MAX);
        // O teto interno não é derivado do público: cada um tem sua autoridade.
        assert_eq!(
            UNION_BINDING_LIMITS.max_regions,
            crate::union_payload::MAX_UNION_BINDING_REGIONS
        );
        assert_eq!(
            UNION_BINDING_LIMITS.max_bytes,
            crate::union_payload::MAX_UNION_BINDING_BYTES
        );
        assert_eq!(
            UNION_BUDGET_LIMITS.max_descriptors,
            crate::union_payload::MAX_UNION_DESCRIPTORS
        );
        assert_eq!(
            PUBLIC_MEMORY_LIMITS.max_identities,
            PUBLIC_MEMORY_MAX_IDENTITIES
        );
    }
}
// @pinker-nav:end interpreter.unioes.contabilidade-dominios

// @pinker-nav:start evidencia.processos.saida-runtime-hospedado
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova no runtime hospedado, com snapshot sintético válido, o round-trip de Resultado<SaidaProcesso, verso> pelos helpers nominais de anexo e carga e a leitura tipada de código, stdout e stderr sem reexecutar processo.
#[cfg(test)]
mod part_d_saida_processo_runtime_tests {
    use super::*;

    fn valor(call: Result<IntrinsicCall, PinkerError>) -> RuntimeValue {
        match call.expect("intrínseca representacional válida") {
            IntrinsicCall::Done(Some(valor)) => valor,
            _ => panic!("intrínseca deveria produzir valor"),
        }
    }

    #[test]
    fn resultado_saida_processo_sintetico_preserva_handle_e_accessors_tipados() {
        let mut memoria_publica = PublicMemoryState::default();
        let mut io = RuntimeIoState {
            open_files: HashMap::new(),
            next_file_handle: 1,
            closed_handles: std::collections::HashSet::new(),
            cli_args: Vec::new(),
            exit_status: None,
        };
        let mut listas = RuntimeListState {
            lists_bombom: HashMap::new(),
            lists_verso: HashMap::new(),
            next_list_handle: 1,
        };
        let mut mapas = novo_runtime_map_state();
        let mut acaso = RuntimeRandomState {
            generators: HashMap::new(),
            next_generator_handle: 1,
        };

        let saida = mapas
            .saidas_processo
            .inserir(crate::saida_processo::SaidaProcesso::nova(
                17,
                "stdout exato".to_string(),
                "stderr exato".to_string(),
            ));
        let resultado = novo_leque(&mut mapas, crate::falha_operacional::TAG_OK);

        let anexado = valor(try_call_intrinsic(
            crate::intrinsics::identity::CalleeIdentity::CompilerInternal,
            crate::enum_payload::ANEXAR_SAIDA_PROCESSO,
            &[
                RuntimeValue::Int(resultado),
                RuntimeValue::SaidaProcesso(saida),
            ],
            &mut memoria_publica,
            &mut io,
            &mut listas,
            &mut mapas,
            &mut acaso,
        ));
        assert_eq!(anexado, RuntimeValue::Int(resultado));

        let carga = valor(try_call_intrinsic(
            crate::intrinsics::identity::CalleeIdentity::CompilerInternal,
            crate::enum_payload::CARGA_SAIDA_PROCESSO,
            &[
                RuntimeValue::Int(resultado),
                RuntimeValue::Int(crate::falha_operacional::TAG_OK),
                RuntimeValue::Int(0),
            ],
            &mut memoria_publica,
            &mut io,
            &mut listas,
            &mut mapas,
            &mut acaso,
        ));
        assert_eq!(carga, RuntimeValue::SaidaProcesso(saida));

        for (acessor, esperado) in [
            (crate::saida_processo::ACESSOR_CODIGO, RuntimeValue::Int(17)),
            (
                crate::saida_processo::ACESSOR_SAIDA,
                RuntimeValue::Str("stdout exato".to_string()),
            ),
            (
                crate::saida_processo::ACESSOR_ERRO,
                RuntimeValue::Str("stderr exato".to_string()),
            ),
        ] {
            assert_eq!(
                valor(try_call_intrinsic(
                    crate::intrinsics::identity::callee_identity_da_grafia_canonica(acessor),
                    acessor,
                    std::slice::from_ref(&carga),
                    &mut memoria_publica,
                    &mut io,
                    &mut listas,
                    &mut mapas,
                    &mut acaso,
                )),
                esperado
            );
        }
    }

    #[test]
    fn erro_operacional_pos_configuracao_nao_cria_snapshot_parcial() {
        let mut listas = RuntimeListState {
            lists_bombom: HashMap::new(),
            lists_verso: HashMap::from([(1, Vec::new())]),
            next_list_handle: 2,
        };
        let mut mapas = novo_runtime_map_state();
        mapas.maps_verso_verso.insert(1, HashMap::new());
        mapas
            .enum_values
            .insert(1, (crate::limite_tempo::TAG_SEM_LIMITE, Vec::new()));
        mapas.next_enum_handle = 2;
        let superficie = crate::falha_operacional::superficie(
            crate::falha_operacional::EXECUTAR_PROCESSO_ESTRUTURADO,
        )
        .expect("autoridade estruturada");
        let chamada = executar_superficie_falivel(
            superficie,
            &[
                RuntimeValue::Str("/executavel/ausente/step3".to_string()),
                RuntimeValue::ListVerso(1),
                RuntimeValue::Str(String::new()),
                RuntimeValue::Str(String::new()),
                RuntimeValue::MapVersoVerso(1),
                RuntimeValue::Int(1),
            ],
            &mut mapas,
            &mut listas,
        )
        .expect("falha de spawn é Resultado::Erro");
        let IntrinsicCall::Done(Some(RuntimeValue::Int(resultado))) = chamada else {
            panic!("superfície deveria devolver Resultado")
        };
        assert_eq!(mapas.saidas_processo.retidos(), 0);
        assert_eq!(
            mapas.enum_values.get(&resultado).map(|(tag, _)| *tag),
            Some(crate::falha_operacional::TAG_ERRO)
        );
    }
}
// @pinker-nav:end evidencia.processos.saida-runtime-hospedado
