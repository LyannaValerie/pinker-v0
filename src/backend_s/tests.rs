//! Módulos de teste do caminho montável, movidos de `src/backend_s.rs` pela
//! unidade BS-3 do inventário da #601 (Task #610).
//!
//! Só o arquivo mudou: as duas regiões cartografadas, os dois módulos e as
//! vinte asserções continuam exatamente como estavam. `super` mudou de
//! significado ao descer um nível, e a reexportação abaixo é a ponte que
//! devolve o pai aos módulos que continuam escritos com `use super::*`.
//! `mod tests` é privado e `#[cfg(test)]`: a ponte não amplia superfície
//! nenhuma para fora do módulo `backend_s`.

pub use super::*;

// @pinker-nav:start evidencia.backend-s.proveniencia-de-ponteiro
// @pinker-nav:domain memoria
// @pinker-nav:layer evidencia
// @pinker-nav:summary Unidade da classificação de proveniência do back-end nativo (continuação do hotfix pós-PR #411): `selected_call_provenance` como autoridade única sobre chamada direta, indireta, por endereço cru e de trato — `Public` quando e somente quando o retorno é ponteiro —, e a regra do cast `virar seta<T>`, que preserva `Public`, `Internal`, `Fabricated` e `Unclassified` tipado como ponteiro, e só produz `Fabricated` a partir de valor não-ponteiro. Cobre os ramos que a superfície da linguagem ainda não alcança, porque `seta<seta<T>>`, carga de ponteiro pela memória e carga de união com ponteiro estão fora do subconjunto atual.
#[cfg(test)]
mod tests_proveniencia_de_ponteiro {
    use super::*;
    use crate::cfg_ir::{OperandIR, TempIR};
    use crate::instr_select::{SelectedBlock, SelectedFunction, SelectedTerminator};

    const PONTEIRO: TypeIR = TypeIR::Pointer { is_volatile: false };
    const OUTRO_PONTEIRO: TypeIR = TypeIR::Pointer { is_volatile: true };

    fn funcao(
        instructions: Vec<SelectedInstr>,
        slot_types: &[(&str, TypeIR)],
        internos: &[&str],
    ) -> SelectedFunction {
        SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: Vec::new(),
            locals: slot_types
                .iter()
                .map(|(nome, _)| nome.to_string())
                .collect(),
            slot_types: slot_types
                .iter()
                .map(|(nome, ty)| (nome.to_string(), *ty))
                .collect(),
            internal_pointer_params: internos.iter().map(|nome| nome.to_string()).collect(),
            blocks: vec![SelectedBlock {
                label: "entrada".to_string(),
                instructions,
                terminator: SelectedTerminator::Ret(None),
            }],
        }
    }

    fn proveniencia(function: &SelectedFunction, temp: TempIR) -> PointerProvenance {
        let mut visiting_temps = HashSet::new();
        let mut visiting_slots = HashSet::new();
        selected_temp_provenance(function, temp, &mut visiting_temps, &mut visiting_slots)
    }

    fn cast(dest: u32, value: OperandIR, target_type: TypeIR) -> SelectedInstr {
        SelectedInstr::Cast {
            dest: TempIR(dest),
            value,
            target_type,
        }
    }

    /// As quatro formas de chamada que devolvem valor, com o mesmo tipo de
    /// retorno, precisam produzir a mesma proveniência. A assimetria anterior
    /// classificava só a direta como `Public`, e o acesso pelas outras descia
    /// sem validação.
    fn chamadas_que_devolvem(ret_type: TypeIR) -> Vec<(&'static str, SelectedInstr)> {
        vec![
            (
                "direta",
                SelectedInstr::Call {
                    dest: TempIR(0),
                    callee: "fabricar".to_string(),
                    args: Vec::new(),
                    ret_type,
                    identidade: crate::intrinsics::identity::CalleeIdentity::User,
                },
            ),
            (
                "indireta",
                SelectedInstr::CallIndirect {
                    dest: TempIR(0),
                    callee: OperandIR::Local("f".to_string()),
                    args: Vec::new(),
                    ret_type,
                },
            ),
            (
                "crua",
                SelectedInstr::CallRaw {
                    dest: Some(TempIR(0)),
                    callee: OperandIR::Local("fp".to_string()),
                    args: Vec::new(),
                    param_types: Vec::new(),
                    ret_type,
                },
            ),
            (
                "trato",
                SelectedInstr::TraitCall {
                    dest: Some(TempIR(0)),
                    object: OperandIR::Local("objeto".to_string()),
                    trait_name: "Fonte".to_string(),
                    method_name: "regiao".to_string(),
                    method_slot: 0,
                    method_count: 1,
                    args: Vec::new(),
                    param_types: Vec::new(),
                    ret_type,
                },
            ),
        ]
    }

    #[test]
    fn toda_forma_de_chamada_que_devolve_ponteiro_e_publica() {
        for (forma, instrucao) in chamadas_que_devolvem(PONTEIRO) {
            let function = funcao(vec![instrucao], &[], &[]);
            assert_eq!(
                proveniencia(&function, TempIR(0)),
                PointerProvenance::Public,
                "chamada {forma} devolvendo ponteiro precisa ser pública"
            );
            assert!(
                proveniencia(&function, TempIR(0)).requires_access_check(),
                "chamada {forma}: o acesso precisa ser validado"
            );
        }
    }

    #[test]
    fn chamada_que_nao_devolve_ponteiro_nunca_e_publica() {
        for (forma, instrucao) in chamadas_que_devolvem(TypeIR::U64) {
            let function = funcao(vec![instrucao], &[], &[]);
            assert_eq!(
                proveniencia(&function, TempIR(0)),
                PointerProvenance::Unclassified,
                "chamada {forma} devolvendo inteiro não pode virar pública"
            );
        }
    }

    /// `alocar` continua público mesmo se o tipo de retorno não for anotado
    /// como ponteiro: é a origem canônica de região pública.
    #[test]
    fn alocar_permanece_publico_pelo_nome() {
        let function = funcao(
            vec![SelectedInstr::Call {
                dest: TempIR(0),
                callee: "alocar".to_string(),
                args: Vec::new(),
                ret_type: TypeIR::U64,
                identidade: crate::intrinsics::identity::callee_identity_da_grafia_canonica(
                    "alocar",
                ),
            }],
            &[],
            &[],
        );
        assert_eq!(
            proveniencia(&function, TempIR(0)),
            PointerProvenance::Public
        );
    }

    #[test]
    fn cast_de_inteiro_para_ponteiro_fabrica_endereco() {
        let function = funcao(vec![cast(0, OperandIR::Int(4096), PONTEIRO)], &[], &[]);
        assert_eq!(
            proveniencia(&function, TempIR(0)),
            PointerProvenance::Fabricated
        );
        assert!(proveniencia(&function, TempIR(0)).requires_access_check());
    }

    #[test]
    fn cast_de_slot_inteiro_para_ponteiro_fabrica_endereco() {
        let function = funcao(
            vec![cast(0, OperandIR::Local("n".to_string()), PONTEIRO)],
            &[("n", TypeIR::U64)],
            &[],
        );
        assert_eq!(
            proveniencia(&function, TempIR(0)),
            PointerProvenance::Fabricated
        );
    }

    /// Contrato central do Ponto 1: em cast ponteiro→ponteiro a proveniência da
    /// origem é preservada, incluindo `Unclassified`. Antes, `Unclassified`
    /// virava `Fabricated` só por falta de informação.
    #[test]
    fn cast_ponteiro_para_ponteiro_preserva_a_proveniencia() {
        // `Public`: chamada que devolve ponteiro.
        let publica = funcao(
            vec![
                SelectedInstr::Call {
                    dest: TempIR(0),
                    callee: "fabricar".to_string(),
                    args: Vec::new(),
                    ret_type: PONTEIRO,
                    identidade: crate::intrinsics::identity::CalleeIdentity::User,
                },
                cast(1, OperandIR::Temp(TempIR(0)), OUTRO_PONTEIRO),
            ],
            &[],
            &[],
        );
        assert_eq!(proveniencia(&publica, TempIR(1)), PointerProvenance::Public);

        // `Internal`: parâmetro de ambiente de closure.
        let interna = funcao(
            vec![cast(
                0,
                OperandIR::Local("__env".to_string()),
                OUTRO_PONTEIRO,
            )],
            &[("__env", PONTEIRO)],
            &["__env"],
        );
        assert_eq!(
            proveniencia(&interna, TempIR(0)),
            PointerProvenance::Internal
        );
        assert!(
            !proveniencia(&interna, TempIR(0)).requires_access_check(),
            "o domínio interno não pode ser confrontado com o registro público"
        );

        // `Fabricated`: cadeia inteiro → ponteiro A → ponteiro B.
        let fabricada = funcao(
            vec![
                cast(0, OperandIR::Int(4096), PONTEIRO),
                cast(1, OperandIR::Temp(TempIR(0)), OUTRO_PONTEIRO),
            ],
            &[],
            &[],
        );
        assert_eq!(
            proveniencia(&fabricada, TempIR(1)),
            PointerProvenance::Fabricated
        );

        // `Unclassified` **tipado como ponteiro**: origem carregada de memória.
        // Este ramo não é alcançável pela superfície atual da linguagem — é
        // exatamente por isso que a evidência é de unidade.
        let nao_classificada = funcao(
            vec![
                SelectedInstr::DerefLoad {
                    dest: TempIR(0),
                    ptr: OperandIR::Local("celula".to_string()),
                    ty: PONTEIRO,
                    is_volatile: false,
                },
                cast(1, OperandIR::Temp(TempIR(0)), OUTRO_PONTEIRO),
            ],
            &[("celula", PONTEIRO)],
            &[],
        );
        assert_eq!(
            proveniencia(&nao_classificada, TempIR(0)),
            PointerProvenance::Unclassified,
            "carga de memória não é classificada pela análise atual"
        );
        assert_eq!(
            proveniencia(&nao_classificada, TempIR(1)),
            PointerProvenance::Unclassified,
            "cast ponteiro→ponteiro não pode promover a classe por falta de informação"
        );
        assert!(
            !proveniencia(&nao_classificada, TempIR(1)).requires_access_check(),
            "a classe não classificada permanece fora da validação pública"
        );
    }

    /// Cadeia longa: ponteiro não classificado atravessa dois casts sem trocar
    /// de classe.
    #[test]
    fn cadeia_de_casts_nao_promove_ponteiro_nao_classificado() {
        let function = funcao(
            vec![
                SelectedInstr::DerefLoad {
                    dest: TempIR(0),
                    ptr: OperandIR::Local("celula".to_string()),
                    ty: PONTEIRO,
                    is_volatile: false,
                },
                cast(1, OperandIR::Temp(TempIR(0)), OUTRO_PONTEIRO),
                cast(2, OperandIR::Temp(TempIR(1)), PONTEIRO),
            ],
            &[("celula", PONTEIRO)],
            &[],
        );
        assert_eq!(
            proveniencia(&function, TempIR(2)),
            PointerProvenance::Unclassified
        );
    }

    /// O resultado de uma comparação é lógico, não ponteiro: convertê-lo em
    /// `seta<T>` fabrica endereço. Confundir o tipo dos operandos com o tipo do
    /// destino faria uma comparação de ponteiros escapar da validação.
    #[test]
    fn comparacao_de_ponteiros_nao_e_ponteiro() {
        let function = funcao(
            vec![
                SelectedInstr::CmpEq {
                    dest: TempIR(0),
                    lhs: OperandIR::Local("a".to_string()),
                    rhs: OperandIR::Local("b".to_string()),
                    ty: PONTEIRO,
                },
                cast(1, OperandIR::Temp(TempIR(0)), PONTEIRO),
            ],
            &[("a", PONTEIRO), ("b", PONTEIRO)],
            &[],
        );
        assert_eq!(
            selected_temp_type(&function, TempIR(0)),
            Some(TypeIR::Logica)
        );
        assert_eq!(
            proveniencia(&function, TempIR(1)),
            PointerProvenance::Fabricated
        );
    }

    /// A autoridade única precisa reconhecer todas as formas de chamada e
    /// recusar o que não é chamada.
    #[test]
    fn autoridade_de_chamada_cobre_as_formas_e_ignora_o_resto() {
        for (forma, instrucao) in chamadas_que_devolvem(PONTEIRO) {
            let shape = selected_call_shape(&instrucao)
                .unwrap_or_else(|| panic!("forma {forma} precisa ser reconhecida"));
            assert_eq!(shape.dest, Some(TempIR(0)));
            assert_eq!(shape.ret_type, PONTEIRO);
        }
        assert!(selected_call_shape(&SelectedInstr::CallVoid {
            callee: "falar".to_string(),
            args: Vec::new(),
            identidade: crate::intrinsics::identity::CalleeIdentity::User,
        })
        .is_none());
        assert!(selected_call_shape(&cast(0, OperandIR::Int(1), PONTEIRO)).is_none());
    }
}
// @pinker-nav:end evidencia.backend-s.proveniencia-de-ponteiro

// @pinker-nav:start evidencia.backend-s.selecao-de-rota-nativa
// @pinker-nav:domain lowering
// @pinker-nav:layer evidencia
// @pinker-nav:summary Unidade da autoridade de seleção de rota do subset externo montável (Issue #522): `resolver_rota_de_chamada` decide entre intrínseca de runtime por aridade, intrínseca por nome, função Pinker declarada e callee desconhecido, e é a mesma decisão consumida por `Call` e `CallVoid`. Cobre as cinco rotas reparadas, a precedência entre autoridades, a recusa de aridade fora do recorte, a não captura de função Pinker ordinária, a rejeição de callee desconhecido e a ausência estrutural das três exclusões `ouvir*` em todas as autoridades de despacho nativo, com probes de sensibilidade reversíveis que ficam vermelhos se o conjunto reconhecido for ampliado.
#[cfg(test)]
mod tests_selecao_de_rota_nativa {
    use super::*;

    /// #532: o unitário passou a nomear a identidade que a resolução entrega.
    /// A rota de builtin exige identidade de builtin; a de função Pinker, não.
    fn rota(callee: &str, argc: usize, declarada: bool) -> RotaDeChamada {
        resolver_rota_de_chamada(
            crate::intrinsics::identity::callee_identity_da_grafia_canonica(callee),
            callee,
            argc,
            || declarada,
        )
    }

    /// A mesma pergunta feita para um callee de USUÁRIO.
    fn rota_de_usuario(callee: &str, argc: usize, declarada: bool) -> RotaDeChamada {
        resolver_rota_de_chamada(
            crate::intrinsics::identity::CalleeIdentity::User,
            callee,
            argc,
            || declarada,
        )
    }

    /// As três identidades que permanecem intencionalmente fora do subset nativo.
    const EXCLUSOES_STDIN: [&str; 3] = ["ouvir", "ouvir_verso", "ouvir_verso_ou"];

    // --- F3: as cinco rotas reparadas resolvem pela autoridade esperada ---

    #[test]
    fn as_cinco_rotas_reparadas_resolvem_para_o_simbolo_de_runtime_esperado() {
        let esperado: [(&str, usize, &str); 6] = [
            ("afirmar", 1, "pinker_afirmar_1"),
            ("afirmar", 2, "pinker_afirmar_2"),
            ("dormir", 1, "pinker_dormir"),
            (
                "emitir_linha_csv_bombom",
                2,
                "pinker_emitir_linha_csv_bombom",
            ),
            ("ler_linha_csv_bombom", 2, "pinker_ler_linha_csv_bombom"),
            ("sair", 1, "pinker_sair"),
        ];
        for (callee, argc, simbolo) in esperado {
            assert_eq!(
                rota(callee, argc, false),
                RotaDeChamada::Runtime(simbolo.to_string()),
                "rota de {callee}/{argc}"
            );
        }
    }

    #[test]
    fn afirmar_despacha_por_aridade_e_recusa_fora_do_recorte() {
        assert_eq!(
            rota("afirmar", 1, false),
            RotaDeChamada::Runtime("pinker_afirmar_1".to_string())
        );
        assert_eq!(
            rota("afirmar", 2, false),
            RotaDeChamada::Runtime("pinker_afirmar_2".to_string())
        );
        for argc in [0, 3, 4] {
            assert_eq!(
                rota("afirmar", argc, false),
                RotaDeChamada::AridadeForaDoRecorte,
                "afirmar/{argc} deve ser recusada"
            );
        }
    }

    /// #532 — a rota nativa é escolhida pela IDENTIDADE, não pela grafia.
    ///
    /// A mesma grafia, com identidade de usuário, tem de sair pela rota da
    /// função Pinker; com identidade de intrínseca, pela rota de runtime. É o
    /// par que distingue "a tabela mudou" de "o portão existe".
    #[test]
    fn a_mesma_grafia_muda_de_rota_quando_a_identidade_muda() {
        for (callee, argc, simbolo) in [
            ("tamanho_verso", 1, "pinker_verso_tamanho"),
            ("ler_arquivo", 1, "pinker_arquivo_ler_bombom"),
            ("afirmar", 1, "pinker_afirmar_1"),
        ] {
            assert_eq!(
                rota(callee, argc, true),
                RotaDeChamada::Runtime(simbolo.to_string()),
                "{callee} com identidade de intrínseca deve ir ao runtime"
            );
            assert_eq!(
                rota_de_usuario(callee, argc, true),
                RotaDeChamada::FuncaoPinker(callee.to_string()),
                "{callee} com identidade de usuário não pode ser capturado pelo runtime"
            );
        }
    }

    /// Sem função Pinker declarada, o callee de usuário não vira intrínseca por
    /// acidente: ele é desconhecido, e o backend recusa.
    #[test]
    fn callee_de_usuario_nao_declarado_nao_cai_na_tabela_de_runtime() {
        for callee in ["tamanho_verso", "ler_arquivo", "afirmar"] {
            assert_eq!(
                rota_de_usuario(callee, 1, false),
                RotaDeChamada::CalleeDesconhecido,
                "{callee}"
            );
        }
    }

    #[test]
    fn funcao_pinker_ordinaria_nao_e_capturada_pelas_rotas_nativas() {
        for callee in ["afirmar_usuario", "dormir_bem", "sair_do_laco", "csv_meu"] {
            assert_eq!(
                rota(callee, 1, true),
                RotaDeChamada::FuncaoPinker(callee.to_string()),
                "{callee} deve permanecer função Pinker"
            );
        }
    }

    #[test]
    fn callee_desconhecido_continua_rejeitado() {
        for callee in ["inexistente_522", "afirmar_usuario", "ouvir"] {
            assert_eq!(
                rota(callee, 1, false),
                RotaDeChamada::CalleeDesconhecido,
                "{callee} sem declaração deve ser recusado"
            );
        }
    }

    #[test]
    fn precedencia_de_autoridade_e_estavel_entre_call_e_callvoid() {
        // A precedência é: aridade, depois nome, depois função Pinker declarada.
        // Uma intrínseca reconhecida não deixa de sê-lo porque existe função
        // homônima declarada; a invalidez dessa declaração é decidida antes,
        // na semântica (#502), e não é relaxada aqui.
        assert_eq!(
            rota("afirmar", 1, true),
            RotaDeChamada::Runtime("pinker_afirmar_1".to_string())
        );
        // Símbolo por nome não é sensível à aridade do call site.
        for argc in [0, 1, 2, 3] {
            assert_eq!(
                rota("sair", argc, false),
                RotaDeChamada::Runtime("pinker_sair".to_string()),
                "sair/{argc}"
            );
        }
    }

    // --- F4: ausência estrutural das exclusões de stdin em TODAS as autoridades ---

    /// Predicado estrutural: nenhuma das três exclusões participa de qualquer
    /// rota nativa alcançável pelo backend, sob o despacho fornecido.
    fn exclusoes_ausentes_de_todas_as_autoridades(
        elegivel_por_aridade: impl Fn(&str) -> bool,
        simbolo_por_aridade: impl Fn(&str, usize) -> Option<String>,
        simbolo_por_nome: impl Fn(&str) -> Option<&'static str>,
    ) -> bool {
        EXCLUSOES_STDIN.iter().all(|callee| {
            !elegivel_por_aridade(callee)
                && (0..=3).all(|argc| simbolo_por_aridade(callee, argc).is_none())
                && simbolo_por_nome(callee).is_none()
        })
    }

    #[test]
    fn exclusoes_de_stdin_ausentes_de_todas_as_autoridades_de_despacho() {
        assert!(
            exclusoes_ausentes_de_todas_as_autoridades(
                is_arity_runtime_intrinsic,
                runtime_intrinsic_symbol_por_aridade,
                runtime_intrinsic_symbol,
            ),
            "ouvir/ouvir_verso/ouvir_verso_ou não podem participar de nenhuma rota nativa"
        );
        // E a decisão composta também as recusa.
        for callee in EXCLUSOES_STDIN {
            assert_eq!(rota(callee, 0, false), RotaDeChamada::CalleeDesconhecido);
            assert_eq!(rota(callee, 1, false), RotaDeChamada::CalleeDesconhecido);
        }
    }

    #[test]
    fn probe_de_sensibilidade_detecta_ouvir_introduzida_no_despacho_por_aridade() {
        // Mutação reversível, local ao teste: um despacho que passa a
        // reconhecer `ouvir` por aridade. O predicado estrutural deve ficar
        // vermelho, provando que ele realmente cobre essa autoridade.
        let mutado_elegivel =
            |callee: &str| callee == "ouvir" || is_arity_runtime_intrinsic(callee);
        let mutado_por_aridade = |callee: &str, argc: usize| {
            if callee == "ouvir" && argc == 0 {
                return Some("pinker_ouvir".to_string());
            }
            runtime_intrinsic_symbol_por_aridade(callee, argc)
        };
        assert!(
            !exclusoes_ausentes_de_todas_as_autoridades(
                mutado_elegivel,
                mutado_por_aridade,
                runtime_intrinsic_symbol,
            ),
            "o predicado precisa ficar vermelho quando ouvir entra pelo despacho por aridade"
        );
    }

    #[test]
    fn probe_de_sensibilidade_detecta_ouvir_introduzida_no_despacho_por_nome() {
        let mutado_por_nome = |callee: &str| -> Option<&'static str> {
            if callee == "ouvir_verso" {
                return Some("pinker_ouvir_verso");
            }
            runtime_intrinsic_symbol(callee)
        };
        assert!(
            !exclusoes_ausentes_de_todas_as_autoridades(
                is_arity_runtime_intrinsic,
                runtime_intrinsic_symbol_por_aridade,
                mutado_por_nome,
            ),
            "o predicado precisa ficar vermelho quando ouvir entra pelo despacho por nome"
        );
    }

    // --- F3: o conjunto reconhecido não é ampliado por acidente ---

    #[test]
    fn conjunto_por_aridade_e_exatamente_o_recorte_autorizado() {
        // Fecha o conjunto nominal do despacho por aridade. Ampliá-lo sem
        // atualizar esta tabela deixa o teste vermelho.
        let nominais_esperados = [
            "afirmar",
            "executar_processo",
            "capturar_stdout",
            "capturar_stderr",
            "executar_com_entrada",
        ];
        for callee in nominais_esperados {
            assert!(
                is_arity_runtime_intrinsic(callee),
                "{callee} deveria ser elegível"
            );
        }
        for callee in [
            "dormir",
            "sair",
            "emitir_linha_csv_bombom",
            "ler_linha_csv_bombom",
        ] {
            assert!(
                !is_arity_runtime_intrinsic(callee),
                "{callee} resolve por nome, não por aridade"
            );
        }
        for callee in EXCLUSOES_STDIN {
            assert!(!is_arity_runtime_intrinsic(callee), "{callee} é exclusão");
        }
    }
}
// @pinker-nav:end evidencia.backend-s.selecao-de-rota-nativa
